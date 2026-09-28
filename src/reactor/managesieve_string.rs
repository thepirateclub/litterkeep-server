//! Сериализация строковых значений ManageSieve в безопасную quoted форму.

/// Экранирует два quoted-special символа RFC 5804 и сохраняет UTF-8.
pub(super) fn quoted(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' | '\\' => output.push('\\'),
            _ => {}
        }
        output.push(character);
    }
    output.push('"');
    output
}

#[cfg(test)]
mod tests {
    use super::quoted;

    /// Ответ сохраняет Unicode и не даёт кавычкам закрыть строку протокола.
    #[test]
    fn escapes_quoted_specials_without_changing_unicode() {
        assert_eq!(quoted("review\"\\папка"), "\"review\\\"\\\\папка\"");
    }
}
