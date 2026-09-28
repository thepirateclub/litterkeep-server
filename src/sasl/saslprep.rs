//! Реализация профиля SASLprep по RFC 4013 и таблицам Stringprep RFC 3454.

#[path = "saslprep_nfkc.rs"]
mod nfkc;

use super::saslprep_data as data;

#[path = "saslprep_tests.rs"]
#[cfg(test)]
mod tests;

const UNASSIGNED: u16 = 1;
const MAP_TO_NOTHING: u16 = 2;
const NON_ASCII_SPACE: u16 = 4;
const RANDAL: u16 = 4096;
const LEFT_TO_RIGHT: u16 = 8192;
const PROHIBITED: u16 = 8 | 16 | 32 | 64 | 128 | 256 | 512 | 1024 | 2048;

/// Различает сохранённую строку и строку-запрос по RFC 3454 §7.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SaslPrepContext {
    /// Новая сохранённая credential не может включать Unicode 3.2 unassigned.
    StoredValue,
    /// Проверяемый клиентский ввод MAY включать Unicode 3.2 unassigned.
    Query,
}

/// Причина отказа подготовки строки SASLprep.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SaslPrepError {
    /// Символ не назначен в репертуаре Unicode 3.2 для сохранённого значения.
    UnassignedCodePoint,
    /// После нормализации присутствует символ из таблиц RFC 4013 §2.3.
    ProhibitedCodePoint,
    /// Строка нарушает обязательные bidi-ограничения RFC 3454 §6.
    BidirectionalRule,
}

/// Применяет mapping, Unicode 3.2 NFKC, prohibited и bidi проверки.
pub fn prepare(value: &str, context: SaslPrepContext) -> Result<String, SaslPrepError> {
    let mapped = map_input(value);
    let normalized = nfkc::normalize(&mapped);
    validate_output(&normalized, context)?;
    Ok(normalized)
}

/// Выполняет RFC 4013 mapping B.1 removal и C.1.2 space replacement.
fn map_input(value: &str) -> String {
    let mut mapped = String::with_capacity(value.len());
    for character in value.chars() {
        let flags = property_flags(u32::from(character));
        match flags & MAP_TO_NOTHING != 0 {
            true => continue,
            false => match flags & NON_ASCII_SPACE != 0 {
                true => mapped.push(' '),
                false => mapped.push(character),
            },
        }
    }
    mapped
}

/// Проверяет запрещённые output, unassigned context и bidi-категории.
fn validate_output(value: &str, context: SaslPrepContext) -> Result<(), SaslPrepError> {
    let mut has_randal = false;
    let mut has_left_to_right = false;
    let mut first_randal = false;
    let mut last_randal = false;
    for (position, character) in value.chars().enumerate() {
        let flags = property_flags(u32::from(character));
        if flags & UNASSIGNED != 0 && context == SaslPrepContext::StoredValue {
            return Err(SaslPrepError::UnassignedCodePoint);
        }
        if flags & PROHIBITED != 0 {
            return Err(SaslPrepError::ProhibitedCodePoint);
        }
        let is_randal = flags & RANDAL != 0;
        has_randal |= is_randal;
        has_left_to_right |= flags & LEFT_TO_RIGHT != 0;
        if position == 0 {
            first_randal = is_randal;
        }
        last_randal = is_randal;
    }
    match (has_randal, has_left_to_right, first_randal, last_randal) {
        (true, false, true, true) => Ok(()),
        (true, _, _, _) => Err(SaslPrepError::BidirectionalRule),
        (false, _, _, _) => Ok(()),
    }
}

/// Ищет объединённые RFC property flags двоичным поиском по generated ranges.
fn property_flags(point: u32) -> u16 {
    let shard_position = data::PROPERTIES
        .partition_point(|shard| shard.last().is_some_and(|(_, end, _)| *end < point));
    data::PROPERTIES
        .get(shard_position)
        .and_then(|shard| {
            shard
                .binary_search_by(|(start, end, _)| compare_range(point, *start, *end))
                .ok()
                .and_then(|position| shard.get(position))
        })
        .map(|(_, _, flags)| *flags)
        .unwrap_or(0)
}

/// Сравнивает кодовую точку с RFC-диапазоном для двоичного поиска.
fn compare_range(point: u32, start: u32, end: u32) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (point.cmp(&start), point.cmp(&end)) {
        (Ordering::Less, _) => Ordering::Greater,
        (_, Ordering::Greater) => Ordering::Less,
        _ => Ordering::Equal,
    }
}
