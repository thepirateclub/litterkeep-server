//! Проверка UTF-8 mailbox names для будущего согласованного режима RFC 9755.

/// Причина отказа для mailbox name, не соответствующего исключениям RFC 9755.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MailboxNameError {
    /// В имени присутствует запрещённый управляющий символ.
    ControlCharacter,
    /// Имя начинается с запрещённого Net-Unicode BOM.
    LeadingByteOrderMark,
}

/// Проверяет ограничения RFC 9755 поверх уже декодированного UTF-8.
///
/// Функция сохраняет исходную последовательность и не нормализует её: RFC 5198
/// рекомендует NFC для передаваемых строк, но принимающая сторона должна быть
/// готова увидеть ненормализованный ввод. Wire-путь пока остаётся legacy UTF-7.
pub fn validate(value: &str) -> Result<(), MailboxNameError> {
    if value.starts_with('\u{feff}') {
        return Err(MailboxNameError::LeadingByteOrderMark);
    }
    match value.chars().any(is_forbidden_control) {
        true => Err(MailboxNameError::ControlCharacter),
        false => Ok(()),
    }
}

/// RFC 9755 запрещает C0, DEL, C1 и Unicode line/paragraph separators.
fn is_forbidden_control(character: char) -> bool {
    matches!(character as u32, 0x0000..=0x001f | 0x007f..=0x009f | 0x2028 | 0x2029)
}

#[cfg(test)]
#[path = "mailbox_utf8_tests.rs"]
mod tests;
