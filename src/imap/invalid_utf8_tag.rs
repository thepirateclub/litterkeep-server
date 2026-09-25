//! Восстановление IMAP command tag после ошибки декодирования wire UTF-8.

/// Сохраняет безопасный ASCII tag, чтобы ответ BAD оставался связан с запросом.
pub(crate) fn extract(line: &[u8]) -> String {
    let tag = line
        .split(u8::is_ascii_whitespace)
        .next()
        .unwrap_or_default();
    match std::str::from_utf8(tag) {
        Ok(value) if super::tokenizer::valid_tag(value) => value.to_owned(),
        Ok(_) | Err(_) => "*".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Повреждённый UTF-8 в аргументе сохраняет валидный tag команды.
    #[test]
    fn invalid_utf8_argument_keeps_safe_command_tag() {
        let wire = b"A17 LOGIN \"b\xffb\" \"password\"\r\n";
        assert_eq!(extract(wire), "A17");
    }

    /// Невалидный сам tag не отражается в ответе как произвольный wire-текст.
    #[test]
    fn invalid_utf8_inside_tag_uses_untagged_fallback() {
        let wire = b"A\xff7 LOGIN \"user\" \"password\"\r\n";
        assert_eq!(extract(wire), "*");
    }

    /// Command tag использует ASTRING-CHAR, поэтому resp-special `]` допустим.
    #[test]
    fn response_special_inside_tag_is_preserved() {
        let wire = b"A]7 LOGIN \"user\xff\" \"password\"\r\n";
        assert_eq!(extract(wire), "A]7");
    }

    /// Формальная грамматика tag исключает plus даже при безопасном ASCII.
    #[test]
    fn plus_inside_tag_uses_untagged_fallback() {
        let wire = b"A+7 LOGIN \"user\xff\" \"password\"\r\n";
        assert_eq!(extract(wire), "*");
    }
}
