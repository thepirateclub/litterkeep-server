//! Декодирование 7-bit-safe формы RFC 6533 utf-8-addr-xtext.

use super::super::ParseError;

/// Преобразует безопасные QCHAR и EmbeddedUnicodeChar в UTF-8 octets.
#[cfg(test)]
pub(super) fn decode(value: &str, maximum: usize) -> Result<Vec<u8>, ParseError> {
    decode_form(value, maximum, false)
}

/// Декодирует RFC 6533 unitext, включая буквальные UTF-8 символы.
pub(super) fn decode_unitext(value: &str, maximum: usize) -> Result<Vec<u8>, ParseError> {
    decode_form(value, maximum, true)
}

fn decode_form(value: &str, maximum: usize, allow_unicode: bool) -> Result<Vec<u8>, ParseError> {
    if value.is_empty() || value.len() > maximum {
        return Err(ParseError::Syntax);
    }
    if !allow_unicode && !value.is_ascii() {
        return Err(ParseError::Syntax);
    }
    let mut input = value.chars().peekable();
    let mut output = Vec::with_capacity(value.len());
    while let Some(character) = input.next() {
        match character {
            '\\' => append_escape(&mut input, &mut output)?,
            character if character.is_ascii() && is_qchar(character as u8) => {
                output.push(character as u8);
            }
            character if allow_unicode && !character.is_ascii() => {
                let mut encoded = [0_u8; 4];
                output.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
            }
            _ => return Err(ParseError::Syntax),
        }
    }
    match std::str::from_utf8(&output) {
        Ok(_) => Ok(output),
        Err(_) => Err(ParseError::Syntax),
    }
}

fn append_escape(
    input: &mut impl Iterator<Item = char>,
    output: &mut Vec<u8>,
) -> Result<(), ParseError> {
    if input.next() != Some('x') || input.next() != Some('{') {
        return Err(ParseError::Syntax);
    }
    let mut digits = String::with_capacity(6);
    loop {
        match input.next() {
            Some('}') => break,
            Some(character) if digits.len() < 6 && character.is_ascii_hexdigit() => {
                digits.push(character);
            }
            _ => return Err(ParseError::Syntax),
        }
    }
    if !(2..=6).contains(&digits.len()) {
        return Err(ParseError::Syntax);
    }
    let point = u32::from_str_radix(&digits, 16).map_err(|_| ParseError::Syntax)?;
    if !is_valid_hexpoint(&digits, point) {
        return Err(ParseError::Syntax);
    }
    let character = char::from_u32(point).ok_or(ParseError::Syntax)?;
    let mut encoded = [0_u8; 4];
    output.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
    Ok(())
}

/// QCHAR RFC 6533: печатный ASCII без SP, обратной косой, плюса и равенства.
fn is_qchar(byte: u8) -> bool {
    matches!(byte, b'!'..=b'*' | b','..=b'<' | b'>'..=b'[' | b']'..=b'~')
}

/// Проверяет длину HEXPOINT и специальные ограничения его ABNF-ветви.
fn is_valid_hexpoint(hex: &str, point: u32) -> bool {
    let grammar = match hex.len() {
        2 => is_two_digit_hexpoint(point),
        3 | 5 => !hex.starts_with('0'),
        4 => is_four_digit_hexpoint(hex),
        6 => hex.starts_with("10"),
        _ => false,
    };
    grammar && is_embedded_point(point)
}

/// Двухзначная ветвь допускает xtext-specials и значения 80..FF.
fn is_two_digit_hexpoint(point: u32) -> bool {
    matches!(
        point,
        0x01..=0x09 | 0x10 | 0x11..=0x19 | 0x20 | 0x2b | 0x3d | 0x5c | 0x7f | 0x80..=0xff
    )
}

/// Четырёхзначная ветвь исключает нулевой префикс и суррогатный диапазон.
fn is_four_digit_hexpoint(hex: &str) -> bool {
    let upper = hex.to_ascii_uppercase();
    match upper.chars().next() {
        Some('D') => matches!(upper.chars().nth(1), Some('0'..='7')),
        Some('0') | None => false,
        Some(_) => true,
    }
}

/// Запрещает NUL, CR/LF и остальные ASCII вне списка xtext-specials.
fn is_embedded_point(point: u32) -> bool {
    match point {
        0x01..=0x09 | 0x10 | 0x11..=0x20 | 0x2b | 0x3d | 0x5c | 0x7f => true,
        0x80..=0x10ffff => char::from_u32(point).is_some(),
        _ => false,
    }
}
