//! Декодирование одного RFC 2047 encoded-word без переписывания raw header.

use super::{charset::Charset, decode_base64, language_tag::LanguageTag};

#[cfg(test)]
#[path = "encoded_word_tests.rs"]
mod tests;

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

/// Декодированный RFC 2047 token вместе с optional RFC 2231 language tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedWord {
    /// Текст после decoding выбранного RFC 2047 charset и B/Q encoding.
    pub text: String,
    /// Optional language metadata из RFC 2231 encoded-word extension.
    pub language: Option<LanguageTag>,
}

/// Декодирует полную форму `=?charset?B|Q?encoded-text?=`.
pub fn decode(word: &str) -> Result<String, &'static str> {
    Ok(decode_with_metadata(word)?.text)
}

/// Декодирует RFC 2047 слово и возвращает языковую подсказку RFC 2231.
pub fn decode_with_metadata(word: &str) -> Result<DecodedWord, &'static str> {
    decode_in_context(word, WordContext::HeaderText)
}

/// Декодирует слово с учётом позиции, в которой RFC разрешает его синтаксис.
fn decode_in_context(word: &str, context: WordContext) -> Result<DecodedWord, &'static str> {
    if word.len() > 75 {
        return Err("encoded-word exceeds 75 characters");
    }
    let inner = word
        .strip_prefix("=?")
        .and_then(|value| value.strip_suffix("?="))
        .ok_or("invalid encoded-word envelope")?;
    let mut sections = inner.splitn(3, '?');
    let charset_field = sections.next().ok_or("missing encoded-word charset")?;
    let (charset_name, language_name) = match charset_field.split_once('*') {
        Some((charset, language)) => (charset, Some(language)),
        None => (charset_field, None),
    };
    let charset = Charset::parse(charset_name)?;
    let language = language_name.map(LanguageTag::parse).transpose()?;
    let encoding = parse_encoding(sections.next().ok_or("missing encoded-word encoding")?)?;
    let encoded = sections.next().ok_or("missing encoded-word text")?;
    validate_encoded_text(encoded)?;
    validate_context_text(context, encoding, encoded)?;
    let bytes = match encoding {
        TransferEncoding::Base64 => decode_base64(encoded.as_bytes())?,
        TransferEncoding::Q => decode_q(encoded.as_bytes())?,
    };
    Ok(DecodedWord {
        text: charset.decode(&bytes)?,
        language,
    })
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
    for word in value.split([' ', '\t']).filter(|word| !word.is_empty()) {
        let decoded = decode_in_context(word, WordContext::Phrase);
        match decoded {
            Ok(decoded) => {
                if !output.is_empty() && !previous_encoded {
                    output.push(' ');
                }
                output.push_str(&decoded.text);
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
        let suffix = value.get(end..).ok_or("invalid encoded-word suffix")?;
        match (
            starts_after_linear_whitespace(prefix, start == 0),
            ends_before_linear_whitespace(suffix),
            decode(candidate),
        ) {
            (true, true, Ok(decoded)) => {
                match (
                    previous_was_encoded,
                    prefix.chars().all(is_linear_whitespace),
                ) {
                    (true, true) => {}
                    _ => output.push_str(prefix),
                }
                output.push_str(&decoded);
                previous_was_encoded = true;
                cursor = end;
            }
            _ => {
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

/// Возвращает true, если токен начинается в начале поля или после LWS.
fn starts_after_linear_whitespace(prefix: &str, at_field_start: bool) -> bool {
    match (at_field_start, prefix.chars().next_back()) {
        (true, _) | (false, Some(' ' | '\t')) => true,
        (false, _) => false,
    }
}

/// Возвращает true, если за encoded-word следует конец поля либо LWS.
fn ends_before_linear_whitespace(suffix: &str) -> bool {
    match suffix.chars().next() {
        None | Some(' ' | '\t') => true,
        Some(_) => false,
    }
}

/// Проверяет один октет RFC 5322 linear whitespace после unfolding.
fn is_linear_whitespace(character: char) -> bool {
    matches!(character, ' ' | '\t')
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
