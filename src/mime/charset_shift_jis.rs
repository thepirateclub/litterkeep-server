//! Строгий Shift_JIS decoder по WHATWG Encoding Standard §12.3.1.

/// Декодирует ASCII, U+0080, half-width katakana и двухбайтовые пары.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    let mut input = bytes.iter().copied();
    let mut output = String::with_capacity(bytes.len());
    while let Some(byte) = input.next() {
        output.push(decode_byte(byte, &mut input)?);
    }
    Ok(output)
}

/// Разбирает самостоятельный октет либо передаёт lead/trail пару decoder-у.
fn decode_byte<I>(byte: u8, input: &mut I) -> Result<char, &'static str>
where
    I: Iterator<Item = u8>,
{
    match byte {
        0x00..=0x80 => Ok(char::from(byte)),
        0xa1..=0xdf => katakana(byte),
        0x81..=0x9f | 0xe0..=0xfc => decode_pair(byte, input),
        _ => Err("invalid Shift_JIS octet"),
    }
}

/// Преобразует half-width katakana без таблицы с входным индексом.
fn katakana(byte: u8) -> Result<char, &'static str> {
    char::from_u32(0xff61 + u32::from(byte - 0xa1)).ok_or("invalid Shift_JIS katakana scalar")
}

/// Валидирует trail octet и вычисляет указатель Shift_JIS.
fn decode_pair<I>(lead: u8, input: &mut I) -> Result<char, &'static str>
where
    I: Iterator<Item = u8>,
{
    let trail = input.next().ok_or("incomplete Shift_JIS sequence")?;
    let pointer = shift_jis_pointer(lead, trail).ok_or("invalid Shift_JIS trailing octet")?;
    match pointer {
        8_836..=10_715 => private_use(pointer),
        _ => super::charset_euc_jp_table::jis0208(pointer)
            .ok_or("unmapped Shift_JIS two-byte sequence"),
    }
}

/// Применяет формулу WHATWG и разделяет две последовательные lead-зоны.
fn shift_jis_pointer(lead: u8, trail: u8) -> Option<usize> {
    let trail_offset = match trail {
        0x40..=0x7e => 0x40,
        0x80..=0xfc => 0x41,
        _ => return None,
    };
    let leading_offset = match lead {
        0x81..=0x9f => 0x81,
        0xe0..=0xfc => 0xc1,
        _ => return None,
    };
    Some(usize::from(lead - leading_offset) * 188 + usize::from(trail - trail_offset))
}

/// Отображает Windows EUDC указатели в supplementary-independent BMP PUA.
fn private_use(pointer: usize) -> Result<char, &'static str> {
    let scalar = 0xe000 + u32::try_from(pointer - 8_836).map_err(|_| "invalid EUDC pointer")?;
    char::from_u32(scalar).ok_or("invalid Shift_JIS private-use scalar")
}

#[cfg(test)]
#[path = "charset_shift_jis_tests.rs"]
mod tests;
