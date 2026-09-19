//! Кодек modified UTF-7 для имён IMAP4rev1 mailbox.
//!
//! Внутренний каталог хранит нормальный Unicode. Этот модуль является
//! единственной границей между ним и wire-представлением RFC 3501: printable
//! ASCII остаётся как есть, ampersand кодируется как `&-`, а остальные
//! последовательности передаются как UTF-16BE в modified Base64.

/// Декодирует wire-имя mailbox в Unicode без неявной подстановки повреждённых
/// символов.
pub fn decode(input: &str) -> Result<String, &'static str> {
    let mut output = String::new();
    let mut remaining = input;
    while let Some(position) = remaining.find('&') {
        let (plain, tail) = remaining.split_at(position);
        validate_direct(plain)?;
        output.push_str(plain);
        let after_marker = tail.strip_prefix('&').ok_or("missing marker")?;
        let Some(end) = after_marker.find('-') else {
            return Err("unterminated modified UTF-7 shift");
        };
        let (encoded, next) = after_marker.split_at(end);
        match encoded.is_empty() {
            true => output.push('&'),
            false => output.push_str(&decode_shift(encoded)?),
        }
        remaining = next.strip_prefix('-').ok_or("missing shift end")?;
    }
    validate_direct(remaining)?;
    output.push_str(remaining);
    Ok(output)
}

/// Кодирует Unicode имя в каноническое wire-представление modified UTF-7.
pub fn encode(input: &str) -> String {
    let mut output = String::new();
    let mut shifted = String::new();
    for character in input.chars() {
        match character {
            '&' => {
                flush_shift(&mut output, &mut shifted);
                output.push_str("&-");
            }
            value if direct(value) => {
                flush_shift(&mut output, &mut shifted);
                output.push(value);
            }
            value => shifted.push(value),
        }
    }
    flush_shift(&mut output, &mut shifted);
    output
}

fn validate_direct(value: &str) -> Result<(), &'static str> {
    match value.chars().all(direct) {
        true => Ok(()),
        false => Err("non-ASCII character outside modified UTF-7 shift"),
    }
}

fn direct(character: char) -> bool {
    matches!(
        (character.is_ascii(), character),
        (true, '\u{20}'..='\u{25}') | (true, '\u{27}'..='\u{7e}')
    )
}

fn flush_shift(output: &mut String, shifted: &mut String) {
    if shifted.is_empty() {
        return;
    }
    let mut bytes = Vec::new();
    for unit in shifted.encode_utf16() {
        bytes.extend(unit.to_be_bytes());
    }
    output.push('&');
    output.push_str(&base64_encode(&bytes));
    output.push('-');
    shifted.clear();
}

fn decode_shift(value: &str) -> Result<String, &'static str> {
    let bytes = base64_decode(value)?;
    if bytes.len() % 2 != 0 {
        return Err("odd UTF-16 byte count");
    }
    let mut units = Vec::new();
    for pair in bytes.chunks_exact(2) {
        let Some(high) = pair.first().copied() else {
            return Err("missing UTF-16 high byte");
        };
        let Some(low) = pair.get(1).copied() else {
            return Err("missing UTF-16 low byte");
        };
        units.push(u16::from_be_bytes([high, low]));
    }
    char::decode_utf16(units)
        .collect::<Result<String, _>>()
        .map_err(|_| "invalid UTF-16 surrogate")
}

fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+,";
    let mut output = String::new();
    for chunk in input.chunks(3) {
        let first = chunk.first().copied().unwrap_or(0);
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(alphabet_byte(ALPHABET, usize::from(first >> 2)));
        output.push(alphabet_byte(
            ALPHABET,
            usize::from((first & 0x03) << 4 | second >> 4),
        ));
        if chunk.len() > 1 {
            output.push(alphabet_byte(
                ALPHABET,
                usize::from((second & 0x0f) << 2 | third >> 6),
            ));
        }
        if chunk.len() > 2 {
            output.push(alphabet_byte(ALPHABET, usize::from(third & 0x3f)));
        }
    }
    output
}

fn alphabet_byte(alphabet: &[u8], position: usize) -> char {
    alphabet
        .get(position)
        .copied()
        .map(char::from)
        .unwrap_or_default()
}

fn base64_decode(input: &str) -> Result<Vec<u8>, &'static str> {
    let values = input
        .bytes()
        .map(base64_value)
        .collect::<Result<Vec<_>, _>>()?;
    if let 1 = values.len() % 4 {
        return Err("invalid modified Base64 length");
    }
    let mut output = Vec::new();
    for chunk in values.chunks(4) {
        let first = chunk.first().copied().ok_or("missing Base64 byte")?;
        let second = chunk.get(1).copied().ok_or("missing Base64 byte")?;
        let third = chunk.get(2).copied().unwrap_or(0);
        let fourth = chunk.get(3).copied().unwrap_or(0);
        output.push(first << 2 | second >> 4);
        if chunk.len() > 2 {
            output.push(second << 4 | third >> 2);
        }
        if chunk.len() > 3 {
            output.push(third << 6 | fourth);
        }
    }
    Ok(output)
}

fn base64_value(value: u8) -> Result<u8, &'static str> {
    match value {
        b'A'..=b'Z' => Ok(value - b'A'),
        b'a'..=b'z' => Ok(value - b'a' + 26),
        b'0'..=b'9' => Ok(value - b'0' + 52),
        b'+' => Ok(62),
        b',' => Ok(63),
        _ => Err("invalid modified Base64 byte"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes_rfc_example() {
        let wire = "&ZeVnLIqe-";
        assert_eq!(decode(wire), Ok("日本語".into()));
        assert_eq!(encode("日本語"), wire);
    }

    #[test]
    fn preserves_ampersand_and_unicode_round_trip() {
        let name = "Проекты & Архив";
        assert_eq!(decode(&encode(name)), Ok(name.into()));
        assert_eq!(encode("A&B"), "A&-B");
    }

    #[test]
    fn rejects_invalid_wire_sequences() {
        for invalid in ["Кириллица", "&", "&A-", "&A,-"] {
            assert!(decode(invalid).is_err(), "accepted {invalid}");
        }
    }
}
