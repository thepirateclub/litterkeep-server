//! NFC-нормализация Net-Unicode по согласованным таблицам Unicode 17.0.0.

#[path = "unicode_17_nfc/data.rs"]
mod data;

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

/// Преобразует строку в Unicode Normalization Form C.
///
/// Каноническая декомпозиция, стабильная сортировка combining classes и
/// canonical composition выполняются раздельно. Hangul обрабатывается
/// алгоритмически, поэтому таблица не дублирует 11 172 слога.
pub(crate) fn normalize(value: &str) -> String {
    match value.is_ascii() {
        true => value.to_owned(),
        false => compose_ordered(canonical_order(decompose(value))),
    }
}

/// Рекурсивно раскрывает canonical mappings явным bounded стеком scalar values.
fn decompose(value: &str) -> Vec<char> {
    let mut result = Vec::with_capacity(value.chars().count());
    for character in value.chars() {
        let mut pending = vec![u32::from(character)];
        while let Some(code_point) = pending.pop() {
            match hangul_decomposition(code_point) {
                Some(mapping) => pending.extend(mapping.into_iter().rev()),
                None => match canonical_decomposition(code_point) {
                    Some(mapping) => pending.extend(mapping.iter().rev().copied()),
                    None => match char::from_u32(code_point) {
                        Some(scalar) => result.push(scalar),
                        None => unreachable!("canonical mappings contain only Unicode scalars"),
                    },
                },
            }
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

/// Добавляет очереди одинакового класса, сохраняя исходный порядок в классе.
fn flush_ordered_marks(ordered: &mut Vec<char>, marks: &mut BTreeMap<u8, Vec<char>>) {
    for (_, class_marks) in std::mem::take(marks) {
        ordered.extend(class_marks);
    }
}

/// Выполняет composition без индексирования: starter удерживается отдельно.
fn compose_ordered(ordered: Vec<char>) -> String {
    let mut output = String::with_capacity(ordered.len());
    let mut starter = None;
    let mut marks = Vec::new();
    let mut previous_class = 0;
    for character in ordered {
        let current_class = combining_class(character);
        match composable_starter(starter, character, previous_class, current_class) {
            Some(composite) => {
                starter = Some(composite);
                continue;
            }
            None => match current_class {
                0 => {
                    flush_composed_segment(&mut output, &mut starter, &mut marks);
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
    flush_composed_segment(&mut output, &mut starter, &mut marks);
    output
}

/// Снимает непрерывный composed segment в выходную строку.
fn flush_composed_segment(output: &mut String, starter: &mut Option<char>, marks: &mut Vec<char>) {
    if let Some(character) = starter.take() {
        output.push(character);
    }
    output.extend(marks.drain(..));
}

/// Проверяет правило blocking и подбирает canonical composite.
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

/// Выполняет алгоритмическое Hangul composition до табличного поиска.
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

fn hangul_decomposition(code_point: u32) -> Option<Vec<u32>> {
    let syllable_index = code_point.checked_sub(S_BASE)?;
    if syllable_index >= S_COUNT {
        return None;
    }
    let leading = L_BASE + syllable_index / N_COUNT;
    let vowel = V_BASE + syllable_index % N_COUNT / T_COUNT;
    let trailing = syllable_index % T_COUNT;
    let mut mapping = vec![leading, vowel];
    if trailing != 0 {
        mapping.push(T_BASE + trailing);
    }
    Some(mapping)
}

fn hangul_leading_index(code_point: u32) -> Option<u32> {
    let index = code_point.checked_sub(L_BASE)?;
    (index < L_COUNT).then_some(index)
}

fn hangul_vowel_index(code_point: u32) -> Option<u32> {
    let index = code_point.checked_sub(V_BASE)?;
    (index < V_COUNT).then_some(index)
}

fn hangul_lv_index(code_point: u32) -> Option<u32> {
    let index = code_point.checked_sub(S_BASE)?;
    match (index < S_COUNT, index % T_COUNT == 0) {
        (true, true) => Some(index / T_COUNT),
        _ => None,
    }
}

fn hangul_trailing_index(code_point: u32) -> Option<u32> {
    let index = code_point.checked_sub(T_BASE)?;
    match (index > 0, index < T_COUNT) {
        (true, true) => Some(index),
        _ => None,
    }
}

fn canonical_decomposition(code_point: u32) -> Option<&'static [u32]> {
    data::DECOMPOSITION.iter().find_map(|shard| {
        shard
            .binary_search_by_key(&code_point, |(candidate, _)| *candidate)
            .ok()
            .and_then(|position| shard.get(position))
            .map(|(_, mapping)| *mapping)
    })
}

fn combining_class(character: char) -> u8 {
    let code_point = u32::from(character);
    data::COMBINING_CLASS
        .iter()
        .find_map(|ranges| {
            ranges
                .binary_search_by(|(start, end, _)| compare_range(code_point, *start, *end))
                .ok()
                .and_then(|position| ranges.get(position))
                .map(|(_, _, class)| *class)
        })
        .unwrap_or(0)
}

fn composition(starter: u32, combining: u32) -> Option<u32> {
    data::COMPOSITION.iter().find_map(|shard| {
        shard
            .binary_search_by(|(left, right, _)| (*left, *right).cmp(&(starter, combining)))
            .ok()
            .and_then(|position| shard.get(position))
            .map(|(_, _, composite)| *composite)
    })
}

fn compare_range(code_point: u32, start: u32, end: u32) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    match (code_point.cmp(&start), code_point.cmp(&end)) {
        (Ordering::Less, _) => Ordering::Greater,
        (_, Ordering::Greater) => Ordering::Less,
        _ => Ordering::Equal,
    }
}

#[cfg(test)]
#[path = "unicode_17_nfc_tests.rs"]
mod tests;
