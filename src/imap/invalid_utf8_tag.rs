//! Восстановление IMAP command tag после ошибки декодирования wire UTF-8.

/// Сохраняет безопасный ASCII tag, чтобы ответ BAD оставался связан с запросом.
pub(crate) fn extract(line: &[u8]) -> String {
    let tag = line
        .split(u8::is_ascii_whitespace)
        .next()
        .unwrap_or_default();
    match std::str::from_utf8(tag) {
        Ok(value) if valid(value) => value.to_owned(),
        Ok(_) | Err(_) => "*".to_owned(),
    }
}

/// Не отражает управляющие и структурные octets обратно в tagged response.
fn valid(tag: &str) -> bool {
    match tag.is_empty() {
        true => false,
        false => tag.bytes().all(valid_byte),
    }
}

/// Проверяет один octet тега по безопасному подмножеству IMAP atom.
fn valid_byte(value: u8) -> bool {
    let structural = matches!(value, b'(' | b')' | b'{' | b'%' | b'*' | b'"' | 92);
    match (value, structural) {
        (0x21..=0x7e, false) => true,
        (_, _) => false,
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
}
