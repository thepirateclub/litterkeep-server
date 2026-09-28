//! Строгий WHATWG EUC-KR decoder с отказом на malformed байтовых парах.

/// Декодирует ASCII и индексированные пары EUC-KR в Unicode.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    let mut input = bytes.iter().copied();
    let mut output = String::with_capacity(bytes.len());
    while let Some(byte) = input.next() {
        output.push(decode_octet(byte, &mut input)?);
    }
    Ok(output)
}

/// Обрабатывает ASCII либо передаёт lead octet обработчику пары.
fn decode_octet<I>(byte: u8, input: &mut I) -> Result<char, &'static str>
where
    I: Iterator<Item = u8>,
{
    match byte {
        0x00..=0x7f => Ok(char::from(byte)),
        0x81..=0xfe => decode_pair(byte, input),
        _ => Err("invalid EUC-KR octet"),
    }
}

/// Проверяет WHATWG trail range и отображает пару через dense index.
fn decode_pair<I>(leading: u8, input: &mut I) -> Result<char, &'static str>
where
    I: Iterator<Item = u8>,
{
    let trailing = input.next().ok_or("incomplete EUC-KR sequence")?;
    match trailing {
        0x41..=0xfe => {}
        _ => return Err("invalid EUC-KR trailing octet"),
    }
    let pointer = usize::from(leading - 0x81) * 190 + usize::from(trailing - 0x41);
    super::charset_euc_kr_table::code_point(pointer).ok_or("unmapped EUC-KR pointer")
}

#[cfg(test)]
#[path = "charset_euc_kr_tests.rs"]
mod tests;
