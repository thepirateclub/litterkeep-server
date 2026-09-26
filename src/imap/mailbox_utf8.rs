//! Проверка UTF-8 mailbox names в согласованном режиме RFC 9755 `UTF8=ACCEPT`.

/// Причина отказа для mailbox name, не соответствующего исключениям RFC 9755.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MailboxNameError {
    /// В имени присутствует запрещённый управляющий символ.
    ControlCharacter,
    /// Имя начинается с запрещённого Net-Unicode BOM.
    LeadingByteOrderMark,
    /// Имя содержит scalar, не назначенный закреплённой версией Unicode.
    UnassignedCodePoint,
}

/// Проверяет RFC 9755 и обязательное ограничение assigned code points RFC 5198.
///
/// Функция сохраняет исходную последовательность и не нормализует её: RFC 5198
/// рекомендует NFC для передаваемых строк, но принимающая сторона должна быть
/// готова увидеть ненормализованный ввод. UCD закреплён как Unicode 17.0.0 и
/// проверяется относительно версии Unicode встроенной в текущий Rust toolchain.
pub fn validate(value: &str) -> Result<(), MailboxNameError> {
    if value.starts_with('\u{feff}') {
        return Err(MailboxNameError::LeadingByteOrderMark);
    }
    match value
        .chars()
        .find(|character| is_forbidden_control(*character))
    {
        Some(_) => Err(MailboxNameError::ControlCharacter),
        None => match value
            .chars()
            .find(|character| !super::unicode_17::is_assigned(*character))
        {
            Some(_) => Err(MailboxNameError::UnassignedCodePoint),
            None => Ok(()),
        },
    }
}

/// RFC 9755 запрещает C0, DEL, C1 и Unicode line/paragraph separators.
fn is_forbidden_control(character: char) -> bool {
    matches!(character as u32, 0x0000..=0x001f | 0x007f..=0x009f | 0x2028 | 0x2029)
}

#[cfg(test)]
#[path = "mailbox_utf8_tests.rs"]
mod tests;
