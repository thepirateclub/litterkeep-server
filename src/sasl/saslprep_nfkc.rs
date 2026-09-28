//! NFKC normalization over Unicode 3.2 data pinned for RFC 4013 SASLprep.

use super::super::saslprep_data as data;

use std::collections::BTreeMap;

const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

/// Применяет Unicode 3.2 NFKC через compatibility decomposition и composition.
pub(super) fn normalize(value: &str) -> String {
    match value.is_ascii() {
        true => value.to_owned(),
        false => compose_ordered(canonical_order(decompose(value))),
    }
}

/// Раскрывает заранее полностью нормализованные NFKD mappings без рекурсии.
fn decompose(value: &str) -> Vec<char> {
    let mut result = Vec::with_capacity(value.chars().count());
    for character in value.chars() {
        match nfkd_mapping(u32::from(character)) {
            Some(mapping) => {
                result.extend(mapping.iter().filter_map(|point| char::from_u32(*point)))
            }
            None => result.push(character),
        }
    }
    result
}

/// Сортирует combining marks устойчиво по canonical combining class.
fn canonical_order(decomposed: Vec<char>) -> Vec<char> {
    let mut ordered = Vec::with_capacity(decomposed.len());
    let mut marks = BTreeMap::<u8, Vec<char>>::new();
    for character in decomposed {
        match combining_class(character) {
            0 => {
                flush_ordered_marks(&mut ordered, &mut marks);
                ordered.push(character);
            }
            class => marks.entry(class).or_default().push(character),
        }
    }
    flush_ordered_marks(&mut ordered, &mut marks);
    ordered
}

/// Сливает отсортированные очереди, сохраняя порядок marks одного класса.
fn flush_ordered_marks(ordered: &mut Vec<char>, marks: &mut BTreeMap<u8, Vec<char>>) {
    for (_, class_marks) in std::mem::take(marks) {
        ordered.extend(class_marks);
    }
}

/// Выполняет композицию, удерживая starter и marks текущего сегмента.
fn compose_ordered(ordered: Vec<char>) -> String {
    let mut output = String::with_capacity(ordered.len());
    let mut starter = None;
    let mut marks = Vec::new();
    let mut previous_class = 0;
    for character in ordered {
        let current_class = combining_class(character);
        match composable_starter(starter, character, previous_class, current_class) {
            Some(composite) => starter = Some(composite),
            None => match current_class {
                0 => {
                    flush_segment(&mut output, &mut starter, &mut marks);
                    starter = Some(character);
                    previous_class = 0;
                }
                class => {
                    marks.push(character);
                    previous_class = class;
                }
            },
        }
    }
    flush_segment(&mut output, &mut starter, &mut marks);
    output
}

/// Добавляет текущий starter и его несоставленные combining marks.
fn flush_segment(output: &mut String, starter: &mut Option<char>, marks: &mut Vec<char>) {
    if let Some(character) = starter.take() {
        output.push(character);
    }
    output.extend(marks.drain(..));
}

/// Проверяет blocking rule и ищет композицию для активного starter.
fn composable_starter(
    starter: Option<char>,
    character: char,
    previous_class: u8,
    current_class: u8,
) -> Option<char> {
    let starter = starter?;
    if previous_class != 0 && previous_class >= current_class {
        return None;
    }
    compose_pair(starter, character)
}

/// Ищет алгоритмическую Hangul composition до табличного lookup.
fn compose_pair(starter: char, combining: char) -> Option<char> {
    let starter_point = u32::from(starter);
    let combining_point = u32::from(combining);
    if let (Some(leading), Some(vowel)) = (
        hangul_leading_index(starter_point),
        hangul_vowel_index(combining_point),
    ) {
        return char::from_u32(S_BASE + (leading * V_COUNT + vowel) * T_COUNT);
    }
    if let (Some(syllable), Some(trailing)) = (
        hangul_lv_index(starter_point),
        hangul_trailing_index(combining_point),
    ) {
        return char::from_u32(S_BASE + syllable * T_COUNT + trailing);
    }
    composition(starter_point, combining_point).and_then(char::from_u32)
}

/// Возвращает индекс Hangul leading jamo для алгоритмической композиции.
fn hangul_leading_index(point: u32) -> Option<u32> {
    let index = point.checked_sub(L_BASE)?;
    (index < L_COUNT).then_some(index)
}

/// Возвращает индекс Hangul vowel jamo для алгоритмической композиции.
fn hangul_vowel_index(point: u32) -> Option<u32> {
    let index = point.checked_sub(V_BASE)?;
    (index < V_COUNT).then_some(index)
}

/// Проверяет, что scalar является Hangul LV-силлабом без trailing jamo.
fn hangul_lv_index(point: u32) -> Option<u32> {
    let index = point.checked_sub(S_BASE)?;
    match (index < S_COUNT, index % T_COUNT == 0) {
        (true, true) => Some(index / T_COUNT),
        _ => None,
    }
}

/// Возвращает ненулевой индекс Hangul trailing jamo.
fn hangul_trailing_index(point: u32) -> Option<u32> {
    let index = point.checked_sub(T_BASE)?;
    match (index > 0, index < T_COUNT) {
        (true, true) => Some(index),
        _ => None,
    }
}

/// Ищет полную NFKD-декомпозицию scalar в отсортированных Unicode 3.2 shards.
fn nfkd_mapping(point: u32) -> Option<&'static [u32]> {
    let shard_position = data::NFKD.partition_point(|shard| {
        shard
            .last()
            .is_some_and(|(last_point, _)| *last_point < point)
    });
    let shard = data::NFKD.get(shard_position)?;
    shard
        .binary_search_by_key(&point, |(candidate, _)| *candidate)
        .ok()
        .and_then(|position| shard.get(position))
        .map(|(_, mapping)| *mapping)
}

/// Возвращает Unicode 3.2 canonical combining class либо класс starter `0`.
fn combining_class(character: char) -> u8 {
    let point = u32::from(character);
    let shard_position = data::COMBINING
        .partition_point(|shard| shard.last().is_some_and(|(_, end, _)| *end < point));
    data::COMBINING
        .get(shard_position)
        .and_then(|shard| {
            shard
                .binary_search_by(|(start, end, _)| compare_range(point, *start, *end))
                .ok()
                .and_then(|position| shard.get(position))
        })
        .map(|(_, _, class)| *class)
        .unwrap_or(0)
}

/// Ищет canonical pair composition с учётом exclusions Unicode 3.2.
fn composition(starter: u32, combining: u32) -> Option<u32> {
    let requested = (starter, combining);
    let shard_position = data::COMPOSITION.partition_point(|shard| {
        shard
            .last()
            .is_some_and(|(left, right, _)| (*left, *right) < requested)
    });
    let shard = data::COMPOSITION.get(shard_position)?;
    shard
        .binary_search_by(|(left, right, _)| (*left, *right).cmp(&requested))
        .ok()
        .and_then(|position| shard.get(position))
        .map(|(_, _, composite)| *composite)
}

/// Сравнивает scalar с диапазоном для корректного двоичного поиска.
fn compare_range(point: u32, start: u32, end: u32) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (point.cmp(&start), point.cmp(&end)) {
        (Ordering::Less, _) => Ordering::Greater,
        (_, Ordering::Greater) => Ordering::Less,
        _ => Ordering::Equal,
    }
}
