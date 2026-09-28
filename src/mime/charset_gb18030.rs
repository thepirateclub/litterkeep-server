//! Строгий MIME decoder GBK/GB18030 по алгоритму WHATWG Encoding Standard.

/// Декодирует ASCII, euro, пары GBK и полные четырёхбайтовые GB18030 scalars.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    let mut input = bytes.iter().copied();
    let mut output = String::with_capacity(bytes.len());
    while let Some(byte) = input.next() {
        output.push(decode_lead(byte, &mut input)?);
    }
    Ok(output)
}

/// Классифицирует самостоятельный ASCII/euro octet либо начинает multibyte sequence.
fn decode_lead<I>(byte: u8, input: &mut I) -> Result<char, &'static str>
where
    I: Iterator<Item = u8>,
{
    match byte {
        0x00..=0x7f => Ok(char::from(byte)),
        0x80 => Ok('€'),
        0x81..=0xfe => decode_after_lead(byte, input),
        _ => Err("invalid GB18030 leading octet"),
    }
}

/// Выбирает two/four-byte форму и отклоняет неполный или malformed ввод.
fn decode_after_lead<I>(lead: u8, input: &mut I) -> Result<char, &'static str>
where
    I: Iterator<Item = u8>,
{
    let Some(second) = input.next() else {
        return Err("incomplete GB18030 sequence");
    };
    match second {
        0x30..=0x39 => decode_four_byte(lead, second, input),
        0x40..=0x7e | 0x80..=0xfe => decode_two_byte(lead, second),
        _ => Err("invalid GB18030 trail octet"),
    }
}

/// Переводит двухбайтовую пару в плотный WHATWG индекс.
fn decode_two_byte(lead: u8, trail: u8) -> Result<char, &'static str> {
    let trail_offset = match trail {
        0x40..=0x7e => 0x40,
        0x80..=0xfe => 0x41,
        _ => return Err("invalid GB18030 two-byte trail"),
    };
    let pointer = usize::from(lead - 0x81) * 190 + usize::from(trail - trail_offset);
    super::charset_gb18030_table::two_byte_code_point(pointer)
        .ok_or("unmapped GB18030 two-byte sequence")
}

/// Декодирует цифровую четвёрку в указатель четырёхбайтовых диапазонов.
fn decode_four_byte<I>(lead: u8, second: u8, input: &mut I) -> Result<char, &'static str>
where
    I: Iterator<Item = u8>,
{
    let Some(third) = input.next() else {
        return Err("incomplete GB18030 sequence");
    };
    let Some(fourth) = input.next() else {
        return Err("incomplete GB18030 sequence");
    };
    match (third, fourth) {
        (0x81..=0xfe, 0x30..=0x39) => {}
        _ => return Err("invalid GB18030 four-byte sequence"),
    }
    let pointer = four_byte_pointer(lead, second, third, fourth);
    super::charset_gb18030_table::four_byte_code_point(pointer)
        .ok_or("unmapped GB18030 four-byte sequence")
}

/// Вычисляет linear pointer согласно формулам раздела 10.2 WHATWG.
fn four_byte_pointer(first: u8, second: u8, third: u8, fourth: u8) -> u32 {
    (u32::from(first - 0x81) * 10 * 126 * 10)
        + (u32::from(second - 0x30) * 10 * 126)
        + (u32::from(third - 0x81) * 10)
        + u32::from(fourth - 0x30)
}

#[cfg(test)]
#[path = "charset_gb18030_tests.rs"]
mod tests;
