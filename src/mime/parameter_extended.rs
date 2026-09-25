//! Декодирование RFC 2231 extended-параметров MIME.

use super::charset::Charset;
use super::parameters::attribute_octet;

/// Разбирает начальный extended value и возвращает его charset и октеты.
///
/// Charset и language разделяются обязательными апострофами; каждый сырой
/// октет payload обязан быть RFC 2231 attribute-char, иначе он percent-кодируется.
pub(super) fn initial(value: &str) -> Result<(Option<Charset>, Vec<u8>), &'static str> {
    let (charset_value, remainder) = value
        .split_once('\'')
        .ok_or("missing extended charset separator")?;
    let (_, payload) = remainder
        .split_once('\'')
        .ok_or("missing extended language separator")?;
    let charset = match charset_value.is_empty() {
        true => None,
        false => Some(Charset::parse(charset_value)?),
    };
    Ok((charset, decode_octets(payload)?))
}

/// Декодирует encoded continuation segment без повторной charset metadata.
pub(super) fn continuation(value: &str) -> Result<Vec<u8>, &'static str> {
    decode_octets(value)
}

/// Преобразует percent-octet escapes и пропускает только RFC 2231 attribute-char.
fn decode_octets(value: &str) -> Result<Vec<u8>, &'static str> {
    let mut decoded = Vec::with_capacity(value.len());
    let raw = value.as_bytes();
    let mut cursor = 0_usize;
    while let Some(byte) = raw.get(cursor).copied() {
        match byte {
            b'%' => {
                let high = raw.get(cursor.saturating_add(1)).copied().and_then(hex);
                let low = raw.get(cursor.saturating_add(2)).copied().and_then(hex);
                let (Some(high), Some(low)) = (high, low) else {
                    return Err("invalid RFC 2231 percent escape");
                };
                decoded.push((high << 4) | low);
                cursor = cursor.saturating_add(3);
            }
            _ if attribute_octet(byte) => {
                decoded.push(byte);
                cursor = cursor.saturating_add(1);
            }
            _ => return Err("invalid RFC 2231 extended value octet"),
        }
    }
    Ok(decoded)
}

/// Декодирует цифры и только заглавные A-F из ext-octet grammar.
fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
