//! Шестнадцатеричное представление UTF-8 ключей без внешней зависимости.

use std::io;

/// Кодирует каждый исходный октет двумя lowercase hex цифрами.
pub(super) fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        let high = usize::from(byte >> 4);
        let low = usize::from(byte & 0x0f);
        if let (Some(high), Some(low)) = (DIGITS.get(high), DIGITS.get(low)) {
            encoded.push(char::from(*high));
            encoded.push(char::from(*low));
        }
    }
    encoded
}

/// Восстанавливает UTF-8, отвергая неоднозначную или повреждённую hex-строку.
pub(super) fn decode_hex(value: &str) -> io::Result<String> {
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    for pair in pairs {
        let high = hex_value(pair[0]);
        let low = hex_value(pair[1]);
        match (high, low) {
            (Some(high), Some(low)) => bytes.push((high << 4) | low),
            _ => return Err(invalid("invalid encoded mailbox key")),
        }
    }
    if !remainder.is_empty() {
        return Err(invalid("odd encoded mailbox key"));
    }
    String::from_utf8(bytes).map_err(|_| invalid("encoded mailbox key is not utf8"))
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
