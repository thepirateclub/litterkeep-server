//! O(1) lookup зарегистрированного WHATWG Big5 decoder index.

/// Magic и размер header asset, сформированного проверяющим генератором.
const HEADER_LENGTH: usize = 8;
/// Число pointer-позиций до максимума 19781 включительно.
const POINTER_COUNT: usize = 19_782;
/// Размер одного serialized Unicode scalar в lookup asset.
const CODE_POINT_LENGTH: usize = 4;
/// Таблица включает magic, declared count и плотный pointer lookup.
const TABLE: &[u8] = include_bytes!("charset_big5_table.bin");

/// Преобразует Big5 pointer в Unicode scalar; нулевой sentinel означает gap.
pub(super) fn code_point(pointer: usize) -> Option<char> {
    if pointer >= POINTER_COUNT {
        return None;
    }
    let offset = pointer.checked_mul(CODE_POINT_LENGTH)?;
    let start = HEADER_LENGTH.checked_add(offset)?;
    let end = start.checked_add(CODE_POINT_LENGTH)?;
    let raw_bytes = TABLE.get(start..end)?;
    let raw = u32::from_le_bytes(raw_bytes.try_into().ok()?);
    if raw == 0 {
        return None;
    }
    char::from_u32(raw)
}

#[cfg(test)]
#[path = "charset_big5_table_tests.rs"]
mod tests;
