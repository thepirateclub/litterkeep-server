//! Канонизация и проверка имён скриптов на границе файлового каталога.

use std::io;

/// Предел RFC 5804 измеряется в Unicode characters, а не октетах UTF-8.
pub(super) const MAX_CHARACTERS: usize = 128;

/// Проверяет входное имя и возвращает единственное NFC-представление ключа.
pub(super) fn canonical(name: &str) -> io::Result<String> {
    validate(name)?;
    let normalized = crate::unicode::nfc::normalize(name);
    validate(&normalized)?;
    Ok(normalized)
}

/// Проверяет длину, назначенность скаляров и запрещённые управляющие знаки.
pub(super) fn validate(name: &str) -> io::Result<()> {
    if name.is_empty() {
        return Err(invalid("unsafe sieve script name"));
    }
    for (position, character) in name.chars().enumerate() {
        if position >= MAX_CHARACTERS {
            return Err(invalid("sieve script name exceeds 128 characters"));
        }
        if !crate::unicode::is_assigned(character) {
            return Err(invalid("sieve script name contains an unassigned scalar"));
        }
        if forbidden_scalar(character, position) {
            return Err(invalid("sieve script name contains a forbidden scalar"));
        }
    }
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Ограничивает имена требованиями RFC 5804 и Net-Unicode.
pub(super) fn forbidden_scalar(character: char, position: usize) -> bool {
    matches!(
        (u32::from(character), position),
        (0x0000..=0x001f, _) | (0x007f..=0x009f, _) | (0x2028..=0x2029, _) | (0xfeff, 0)
    )
}
