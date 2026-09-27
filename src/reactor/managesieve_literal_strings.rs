//! Общие границы quoted string и command atom для ManageSieve literals.

/// Отделяет первый command atom от оставшихся аргументов.
pub(super) fn split_word(text: &str) -> (&str, &str) {
    let trimmed = text.trim_start();
    match trimmed.find(char::is_whitespace) {
        Some(position) => match trimmed.get(..position) {
            Some(verb) => (
                verb,
                trimmed.get(position..).unwrap_or_default().trim_start(),
            ),
            None => ("", ""),
        },
        None => (trimmed, ""),
    }
}
