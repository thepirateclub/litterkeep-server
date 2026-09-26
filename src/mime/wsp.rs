//! Ограниченная обработка whitespace для структурированных MIME значений.

/// Удаляет внешние RFC 5322 WSP, не поглощая Unicode whitespace.
///
/// В грамматике RFC 5322 WSP задан как SP или HTAB. Unicode-пробелы могут
/// быть данными внутри quoted-string, но не заменяют синтаксические разделители.
pub(crate) fn trim(value: &str) -> &str {
    value.trim_matches(|character| matches!(character, ' ' | '\t'))
}
