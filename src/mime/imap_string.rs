//! Кодирование IMAP string/nstring с выбором безопасной wire-формы.

/// Возвращает NIL либо кодирует значение quoted-string или server literal.
///
/// RFC 9051 допускает NUL и CRLF в literal, но исключает их из quoted-string.
/// Поэтому такие данные передаются synchronizing literal с точным размером
/// UTF-8 в октетах, а обычные значения остаются компактными quoted-string.
pub(super) fn encode_nstring(value: Option<String>) -> String {
    let Some(value) = value else {
        return "NIL".to_owned();
    };
    match requires_literal(&value) {
        true => format!("{{{}}}\r\n{value}", value.len()),
        false => encode_quoted(&value),
    }
}

fn requires_literal(value: &str) -> bool {
    value
        .chars()
        .any(|character| matches!(character, '\0' | '\r' | '\n'))
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

    /// Проверяет escaping обычных строк без изменения их UTF-8 содержимого.
    #[test]
    fn quotes_regular_unicode_and_special_characters() {
        assert_eq!(encode_nstring(None), "NIL");
        assert_eq!(
            encode_nstring(Some("nœde \"quoted\" \\".to_owned())),
            "\"nœde \\\"quoted\\\" \\\\\""
        );
    }

    /// Проверяет literal framing для запрещённых в quoted-string октетов.
    #[test]
    fn uses_octet_counted_literals_for_nul_and_newlines() {
        assert_eq!(encode_nstring(Some("a\0b".to_owned())), "{3}\r\na\0b");
        assert_eq!(
            encode_nstring(Some("œ\r\nline".to_owned())),
            "{8}\r\nœ\r\nline"
        );
    }
}
