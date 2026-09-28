//! Быстрый доступ к проверенной WHATWG двухбайтовой таблице GB18030.

/// Начало таблиц после четырёх байтов magic и двух little-endian счётчиков.
const HEADER_LENGTH: usize = 12;
/// Число указателей двухбайтового индекса из закреплённого WHATWG файла.
const TWO_BYTE_POINTER_COUNT: usize = 23940;
/// Число пар начала диапазонов supplementary-plane индекса.
const RANGE_COUNT: usize = 207;
/// Длина одного little-endian code point в плотной таблице.
const CODE_POINT_LENGTH: usize = 4;
/// Длина записи указатель/code point в таблице четырёхбайтовых диапазонов.
const RANGE_RECORD_LENGTH: usize = 8;
/// Объединённый ресурс создаётся скриптом с проверкой SHA-256 исходных индексов.
const TABLES: &[u8] = include_bytes!("charset_gb18030_tables.bin");

/// Возвращает Unicode scalar для двухбайтового указателя либо `None` для gap.
pub(super) fn two_byte_code_point(pointer: usize) -> Option<char> {
    if pointer >= TWO_BYTE_POINTER_COUNT {
        return None;
    }
    let byte_offset = pointer.checked_mul(CODE_POINT_LENGTH)?;
    let start = HEADER_LENGTH.checked_add(byte_offset)?;
    let end = start.checked_add(CODE_POINT_LENGTH)?;
    let bytes = TABLES.get(start..end)?;
    let raw = u32::from_le_bytes(bytes.try_into().ok()?);
    if raw == 0 {
        return None;
    }
    char::from_u32(raw)
}

/// Находит последний диапазон, начало которого не превосходит указатель.
pub(super) fn four_byte_code_point(pointer: u32) -> Option<char> {
    match pointer {
        39_420..=188_999 | 1_237_576..=u32::MAX => return None,
        _ => {}
    }
    if pointer == 7_457 {
        return char::from_u32(0xe7c7);
    }
    let range_index = find_range(pointer)?;
    let record = read_range(range_index)?;
    let code_point = record.1.checked_add(pointer.checked_sub(record.0)?)?;
    char::from_u32(code_point)
}

/// Выполняет двоичный поиск по компактным записям без индексирования массива.
fn find_range(pointer: u32) -> Option<usize> {
    let mut lower = 0_usize;
    let mut upper = RANGE_COUNT;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        let (range_pointer, _) = read_range(middle)?;
        match range_pointer.cmp(&pointer) {
            std::cmp::Ordering::Less | std::cmp::Ordering::Equal => lower = middle + 1,
            std::cmp::Ordering::Greater => upper = middle,
        }
    }
    lower.checked_sub(1)
}

/// Читает запись только через проверенный диапазон среза.
fn read_range(index: usize) -> Option<(u32, u32)> {
    if index >= RANGE_COUNT {
        return None;
    }
    let data_start = HEADER_LENGTH.checked_add(TWO_BYTE_POINTER_COUNT.checked_mul(4)?)?;
    let start = data_start.checked_add(index.checked_mul(RANGE_RECORD_LENGTH)?)?;
    let end = start.checked_add(RANGE_RECORD_LENGTH)?;
    let bytes = TABLES.get(start..end)?;
    let (pointer_bytes, code_point_bytes) = bytes.split_at(4);
    let pointer = u32::from_le_bytes(pointer_bytes.try_into().ok()?);
    let code_point = u32::from_le_bytes(code_point_bytes.try_into().ok()?);
    Some((pointer, code_point))
}

#[cfg(test)]
#[path = "charset_gb18030_table_tests.rs"]
mod tests;
