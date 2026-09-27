//! Проверка CRLF-представления 7BIT и 8BITMIME SMTP DATA.

/// Запрещённые октеты MIME DATA, отличные от лимита строки или сообщения.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InvalidLineOctet {
    Nul,
    BareCarriageReturnOrLineFeed,
}

/// Требует NUL-free octets и парные CRLF без изменения исходных байтов.
pub(super) fn validate_line_octets(content: &[u8]) -> Result<(), InvalidLineOctet> {
    let mut previous_was_cr = false;
    for octet in content {
        match (*octet, previous_was_cr) {
            (0, _) => return Err(InvalidLineOctet::Nul),
            (b'\r', false) => previous_was_cr = true,
            (b'\n', true) => previous_was_cr = false,
            (b'\n', false) | (_, true) => {
                return Err(InvalidLineOctet::BareCarriageReturnOrLineFeed);
            }
            (_, false) => {}
        }
    }
    match previous_was_cr {
        true => Err(InvalidLineOctet::BareCarriageReturnOrLineFeed),
        false => Ok(()),
    }
}

#[cfg(test)]
#[path = "smtp_data_octets_tests.rs"]
mod tests;
