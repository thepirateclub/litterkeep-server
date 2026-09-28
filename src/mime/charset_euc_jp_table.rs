//! O(1) доступ к WHATWG JIS X 0208 и X 0212 pointer indexes.

const HEADER_LENGTH: usize = 12;
const JIS0208_POINTER_COUNT: usize = 11_104;
const JIS0212_POINTER_COUNT: usize = 8_836;
const CODE_POINT_LENGTH: usize = 4;
const TABLE: &[u8] = include_bytes!("charset_euc_jp_tables.bin");

/// Возвращает scalar из одного из двух плотных указателей WHATWG.
fn code_point(table_number: usize, pointer: usize) -> Option<char> {
    let (table_offset, pointer_count) = match table_number {
        0 => (0, JIS0208_POINTER_COUNT),
        1 => (
            JIS0208_POINTER_COUNT.checked_mul(CODE_POINT_LENGTH)?,
            JIS0212_POINTER_COUNT,
        ),
        _ => return None,
    };
    if pointer >= pointer_count {
        return None;
    }
    let offset = HEADER_LENGTH
        .checked_add(table_offset)?
        .checked_add(pointer.checked_mul(CODE_POINT_LENGTH)?)?;
    let bytes = TABLE.get(offset..offset.checked_add(CODE_POINT_LENGTH)?)?;
    let value = u32::from_le_bytes(bytes.try_into().ok()?);
    match value {
        0 => None,
        _ => char::from_u32(value),
    }
}

pub(super) fn jis0208(pointer: usize) -> Option<char> {
    code_point(0, pointer)
}

pub(super) fn jis0212(pointer: usize) -> Option<char> {
    code_point(1, pointer)
}

#[cfg(test)]
#[path = "charset_euc_jp_table_tests.rs"]
mod tests;
