//! Кодирование IMAP string/nstring с выбором безопасной wire-формы.

/// Возвращает NIL либо кодирует значение quoted-string или server literal.
///
/// RFC 9051 допускает NUL и CRLF в literal, но исключает их из quoted-string.
/// Поэтому такие данные передаются synchronizing literal с точным размером
/// UTF-8 в октетах, а обычные значения остаются компактными quoted-string.
pub(super) fn encode_nstring(
    value: Option<String>,
    mode: crate::imap::tokenizer::StringMode,
) -> String {
    let Some(value) = value else {
        return "NIL".to_owned();
    };
    match requires_literal(&value, mode) {
        true => format!("{{{}}}\r\n{value}", value.len()),
        false => encode_quoted(&value),
    }
}

/// RFC 3501 literals сохраняют Unicode, не помещая high-bit в quoted-string.
fn requires_literal(value: &str, mode: crate::imap::tokenizer::StringMode) -> bool {
    value.chars().any(|character| match (character, mode) {
        ('\0' | '\r' | '\n', _) => true,
        (value, crate::imap::tokenizer::StringMode::Legacy) => !value.is_ascii(),
        (_, crate::imap::tokenizer::StringMode::Utf8Accept) => false,
    })
}

fn encode_quoted(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' | '"' => {
                output.push('\\');
                output.push(character);
            }
            _ => output.push(character),
        }
    }
    output.push('"');
    output
}

#[cfg(test)]
mod tests {
    use super::encode_nstring;
    use crate::imap::tokenizer::StringMode;

    /// Проверяет escaping обычных строк без изменения их UTF-8 содержимого.
    #[test]
    fn quotes_regular_unicode_and_special_characters() {
        assert_eq!(encode_nstring(None, StringMode::Legacy), "NIL");
        assert_eq!(
            encode_nstring(Some("node \"quoted\" \\".to_owned()), StringMode::Legacy),
            "\"node \\\"quoted\\\" \\\\\""
        );
        assert_eq!(
            encode_nstring(Some("nœde".to_owned()), StringMode::Legacy),
            "{5}\r\nnœde"
        );
        assert_eq!(
            encode_nstring(Some("nœde".to_owned()), StringMode::Utf8Accept),
            "\"nœde\""
        );
    }

    /// Проверяет literal framing для запрещённых в quoted-string октетов.
    #[test]
    fn uses_octet_counted_literals_for_nul_and_newlines() {
        assert_eq!(
            encode_nstring(Some("a\0b".to_owned()), StringMode::Utf8Accept),
            "{3}\r\na\0b"
        );
        assert_eq!(
            encode_nstring(Some("œ\r\nline".to_owned()), StringMode::Utf8Accept),
            "{8}\r\nœ\r\nline"
        );
    }
}
