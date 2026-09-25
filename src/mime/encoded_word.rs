//! Декодирование одного RFC 2047 encoded-word без переписывания raw header.

use super::{charset::Charset, decode_base64};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransferEncoding {
    Base64,
    Q,
}

/// Контекст определяет дополнительные ограничения RFC 2047 для encoded-word.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WordContext {
    HeaderText,
    Phrase,
}

/// Декодирует полную форму `=?charset?B|Q?encoded-text?=`.
pub fn decode(word: &str) -> Result<String, &'static str> {
    decode_in_context(word, WordContext::HeaderText)
}

/// Декодирует слово с учётом позиции, в которой RFC разрешает его синтаксис.
fn decode_in_context(word: &str, context: WordContext) -> Result<String, &'static str> {
    if word.len() > 75 {
        return Err("encoded-word exceeds 75 characters");
    }
    let inner = word
        .strip_prefix("=?")
        .and_then(|value| value.strip_suffix("?="))
        .ok_or("invalid encoded-word envelope")?;
    let mut sections = inner.splitn(3, '?');
    let charset = Charset::parse(sections.next().ok_or("missing encoded-word charset")?)?;
    let encoding = parse_encoding(sections.next().ok_or("missing encoded-word encoding")?)?;
    let encoded = sections.next().ok_or("missing encoded-word text")?;
    validate_encoded_text(encoded)?;
    validate_context_text(context, encoding, encoded)?;
    let bytes = match encoding {
        TransferEncoding::Base64 => decode_base64(encoded.as_bytes())?,
        TransferEncoding::Q => decode_q(encoded.as_bytes())?,
    };
    charset.decode(&bytes)
}

/// Применяет ограничения, которые зависят от поля и грамматической позиции.
fn validate_context_text(
    context: WordContext,
    encoding: TransferEncoding,
    encoded: &str,
) -> Result<(), &'static str> {
    match (context, encoding) {
        (WordContext::Phrase, TransferEncoding::Q) => validate_phrase_q_text(encoded),
        _ => Ok(()),
    }
}

/// Проверяет разрешённый RFC 2047 §5 алфавит Q-encoded-word внутри phrase.
fn validate_phrase_q_text(value: &str) -> Result<(), &'static str> {
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        match byte {
            b'=' => match (bytes.next().and_then(hex), bytes.next().and_then(hex)) {
                (Some(_), Some(_)) => {}
                _ => return Err("invalid Q escape in phrase encoded-word"),
            },
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'!' | b'*' | b'+' | b'-' | b'/' | b'_' => {}
            _ => return Err("character is not allowed in phrase Q encoded-word"),
        }
    }
    Ok(())
}

/// Проверяет RFC 2047 encoded-text до передачи данных B/Q decoder-у.
///
/// Грамматика требует хотя бы один печатный ASCII octet и запрещает пробел и
/// вопросительный знак. В частности, пустое слово не становится валидным
/// только потому, что transfer decoder может вернуть пустое тело.
fn validate_encoded_text(value: &str) -> Result<(), &'static str> {
    match value.is_empty() {
        true => Err("empty encoded-word text"),
        false => match value
            .bytes()
            .all(|byte| (33..=126).contains(&byte) && byte != b'?')
        {
            true => Ok(()),
            false => Err("invalid RFC 2047 encoded-text octet"),
        },
    }
}

/// Декодирует encoded-word только в позиции отдельного слова RFC 2047 phrase.
///
/// Некорректное либо неподдерживаемое слово остаётся обычным текстом: RFC 2047
/// запрещает из-за него отклонять или скрывать всё сообщение.
pub fn decode_phrase(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut previous_encoded = false;
    for word in value.split_whitespace() {
        let decoded = decode_in_context(word, WordContext::Phrase);
        match decoded {
            Ok(decoded) => {
                if !output.is_empty() && !previous_encoded {
                    output.push(' ');
                }
                output.push_str(&decoded);
                previous_encoded = true;
            }
            Err(_) => {
                if !output.is_empty() {
                    output.push(' ');
                }
                output.push_str(word);
                previous_encoded = false;
            }
        }
    }
    output
}

/// Декодирует все encoded-word в неструктурированном header field.
pub fn decode_header(value: &str) -> Result<String, &'static str> {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0_usize;
    let mut previous_was_encoded = false;
    while let Some(relative_start) = value.get(cursor..).and_then(|tail| tail.find("=?")) {
        let start = cursor.saturating_add(relative_start);
        let prefix = value
            .get(cursor..start)
            .ok_or("invalid encoded-word prefix")?;
        let search_start = start.saturating_add(2);
        let Some(relative_end) = value.get(search_start..).and_then(|tail| tail.find("?=")) else {
            output.push_str(value.get(cursor..).ok_or("invalid header tail")?);
            return Ok(output);
        };
        let end = search_start.saturating_add(relative_end).saturating_add(2);
        let candidate = value.get(start..end).ok_or("invalid encoded-word range")?;
        match decode(candidate) {
            Ok(decoded) => {
                match (
                    previous_was_encoded,
                    prefix.chars().all(char::is_whitespace),
                ) {
                    (true, true) => {}
                    _ => output.push_str(prefix),
                }
                output.push_str(&decoded);
                previous_was_encoded = true;
                cursor = end;
            }
            Err(_) => {
                output.push_str(
                    value
                        .get(cursor..search_start)
                        .ok_or("invalid raw prefix")?,
                );
                previous_was_encoded = false;
                cursor = search_start;
            }
        }
    }
    output.push_str(value.get(cursor..).ok_or("invalid final header range")?);
    Ok(output)
}

fn parse_encoding(value: &str) -> Result<TransferEncoding, &'static str> {
    match value.to_ascii_uppercase().as_str() {
        "B" => Ok(TransferEncoding::Base64),
        "Q" => Ok(TransferEncoding::Q),
        _ => Err("unsupported encoded-word transfer encoding"),
    }
}

fn decode_q(input: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut output = Vec::with_capacity(input.len());
    let mut cursor = 0_usize;
    while let Some(byte) = input.get(cursor).copied() {
        match byte {
            b'_' => output.push(b' '),
            b'=' => {
                let high = input.get(cursor.saturating_add(1)).copied().and_then(hex);
                let low = input.get(cursor.saturating_add(2)).copied().and_then(hex);
                let (Some(high), Some(low)) = (high, low) else {
                    return Err("invalid encoded-word Q escape");
                };
                output.push((high << 4) | low);
                cursor = cursor.saturating_add(2);
            }
            b'?' | b' ' | b'\t' => return Err("invalid encoded-word Q character"),
            _ => output.push(byte),
        }
        cursor = cursor.saturating_add(1);
    }
    Ok(output)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_base64_utf8_and_q_latin1() {
        assert_eq!(
            decode("=?UTF-8?B?0J/RgNC40LLQtdGC?=").expect("base64"),
            "Привет"
        );
        assert_eq!(
            decode("=?ISO-8859-1?Q?caf=E9_file?=").expect("q"),
            "café file"
        );
    }

    #[test]
    fn rejects_unknown_charset_and_bad_q_escape() {
        assert_eq!(
            decode("=?windows-1251?Q?=CF=F0=E8=E2=E5=F2?=").expect("cp1251"),
            "Привет"
        );
        assert!(decode("=?UTF-8?Q?bad=ZZ?=").is_err());
    }

    #[test]
    fn rejects_empty_and_non_printable_encoded_text() {
        for word in [
            "=?UTF-8?B??=",
            "=?UTF-8?Q??=",
            "=?UTF-8?Q?white space?=",
            "=?UTF-8?Q?tab\tinside?=",
            "=?UTF-8?Q?\u{0001}?=",
            "=?UTF-8?Q?\u{00e9}?=",
        ] {
            assert!(decode(word).is_err(), "accepted invalid word {word:?}");
        }
        assert_eq!(
            decode("=?UTF-8?Q?control=01?=").expect("encoded octet"),
            "control\u{0001}"
        );
    }

    #[test]
    fn invalid_word_remains_visible_in_header_projection() {
        let word = "=?UTF-8?Q?tab\tinside?=";
        assert_eq!(decode_header(word).expect("ordinary header text"), word);
    }

    #[test]
    fn phrase_q_encoding_uses_its_restricted_character_set() {
        let invalid_phrase_word = "=?UTF-8?Q?Smith.Jones?=";
        assert_eq!(decode(invalid_phrase_word), Ok("Smith.Jones".to_owned()));
        assert_eq!(decode_phrase(invalid_phrase_word), invalid_phrase_word);
        assert_eq!(
            decode_phrase("=?UTF-8?Q?Smith=2EJones!_*+-/?="),
            "Smith.Jones! *+-/"
        );
        assert_eq!(decode_phrase("=?UTF-8?B?U21pdGguSm9uZXM=?="), "Smith.Jones");
    }

    #[test]
    fn decodes_adjacent_words_and_preserves_plain_prefix() {
        let value = "Re: =?UTF-8?B?0J/RgNC40LLQtdGC?= =?ISO-8859-1?Q?_caf=E9?=";
        assert_eq!(decode_header(value).expect("header"), "Re: Привет café");
    }
}
