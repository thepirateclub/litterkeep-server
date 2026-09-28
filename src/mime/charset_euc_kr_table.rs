//! O(1) доступ к закреплённому WHATWG index EUC-KR.

/// Размер служебного заголовка binary lookup asset.
const HEADER_LENGTH: usize = 8;
/// Количество pointer positions из нормативного WHATWG индекса.
pub(super) const POINTER_COUNT: usize = 23_940;
/// Число байтов в little-endian представлении Unicode scalar.
const SCALAR_LENGTH: usize = 4;
/// Индекс создан скриптом с проверкой SHA-256 исходного WHATWG файла.
const TABLE: &[u8] = include_bytes!("charset_euc_kr_table.bin");

/// Возвращает Unicode scalar позиции или `None` для незаданного pointer.
pub(super) fn code_point(pointer: usize) -> Option<char> {
    if pointer >= POINTER_COUNT {
        return None;
    }
    let start = HEADER_LENGTH.checked_add(pointer.checked_mul(SCALAR_LENGTH)?)?;
    let end = start.checked_add(SCALAR_LENGTH)?;
    let encoded = TABLE.get(start..end)?;
    let scalar = u32::from_le_bytes(encoded.try_into().ok()?);
    match scalar {
        0 => None,
        value => char::from_u32(value),
    }
}

/// Проверяет сигнатуру, объявленную длину и размер собранного asset.
#[cfg(test)]
pub(super) fn asset_is_well_formed() -> bool {
    let magic_matches = TABLE.get(..4) == Some(b"LKEK".as_slice());
    let count_matches = TABLE
        .get(4..HEADER_LENGTH)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(u32::from_le_bytes)
        == u32::try_from(POINTER_COUNT).ok();
    let expected_size = HEADER_LENGTH.checked_add(POINTER_COUNT * SCALAR_LENGTH);
    let size_matches = expected_size == Some(TABLE.len());
    matches!(
        (magic_matches, count_matches, size_matches),
        (true, true, true)
    )
}

#[cfg(test)]
#[path = "charset_euc_kr_table_tests.rs"]
mod tests;
