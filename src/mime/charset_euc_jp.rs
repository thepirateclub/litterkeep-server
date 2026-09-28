//! Строгий EUC-JP decoder по WHATWG Encoding Standard §12.1.1.

/// Декодирует ASCII, kana, JIS X 0208 и JIS X 0212 формы EUC-JP.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    let mut input = bytes.iter().copied();
    let mut output = String::with_capacity(bytes.len());
    while let Some(byte) = input.next() {
        match byte {
            0x00..=0x7f => output.push(char::from(byte)),
            0x8e => output.push(decode_kana(&mut input)?),
            0x8f => output.push(decode_jis0212(&mut input)?),
            0xa1..=0xfe => output.push(decode_jis0208(byte, &mut input)?),
            _ => return Err("invalid EUC-JP leading octet"),
        }
    }
    Ok(output)
}

/// Преобразует SS2 + A1..DF в half-width katakana.
fn decode_kana<I>(input: &mut I) -> Result<char, &'static str>
where
    I: Iterator<Item = u8>,
{
    let trail = input.next().ok_or("incomplete EUC-JP kana sequence")?;
    match trail {
        0xa1..=0xdf => {
            char::from_u32(0xff61 + u32::from(trail - 0xa1)).ok_or("invalid EUC-JP kana scalar")
        }
        _ => Err("invalid EUC-JP kana trailing octet"),
    }
}

/// Декодирует стандартную двухбайтовую JIS X 0208 область.
fn decode_jis0208<I>(lead: u8, input: &mut I) -> Result<char, &'static str>
where
    I: Iterator<Item = u8>,
{
    let trail = index_octet(input.next())?;
    let pointer = usize::from(lead - 0xa1) * 94 + usize::from(trail);
    super::charset_euc_jp_table::jis0208(pointer).ok_or("unmapped EUC-JP JIS 0208 sequence")
}

/// Декодирует SS3 и JIS X 0212 указатель.
fn decode_jis0212<I>(input: &mut I) -> Result<char, &'static str>
where
    I: Iterator<Item = u8>,
{
    let lead = index_octet(input.next())?;
    let trail = index_octet(input.next())?;
    let pointer = usize::from(lead) * 94 + usize::from(trail);
    super::charset_euc_jp_table::jis0212(pointer).ok_or("unmapped EUC-JP JIS 0212 sequence")
}

/// Проверяет диапазон A1..FE и возвращает zero-based index slot.
fn index_octet(octet: Option<u8>) -> Result<u8, &'static str> {
    match octet {
        Some(value @ 0xa1..=0xfe) => Ok(value - 0xa1),
        Some(_) => Err("invalid EUC-JP trailing octet"),
        None => Err("incomplete EUC-JP multibyte sequence"),
    }
}

#[cfg(test)]
#[path = "charset_euc_jp_tests.rs"]
mod tests;
