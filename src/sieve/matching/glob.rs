//! Glob сопоставление Sieve с byte-captures для RFC 5229 Variables.

use crate::sieve::ast::Comparator;
use std::ops::Range;

/// Ограничивает число переходов даже для специально подобранных wildcard-ов.
const MAX_GLOB_STEPS: usize = 1_000_000;

/// Сопоставляет полную строку и возвращает captures wildcard-ов по порядку.
pub(super) fn captures(
    value: &[u8],
    pattern: &[u8],
    comparator: &Comparator,
) -> Result<Option<Vec<Vec<u8>>>, GlobError> {
    let (tokens, capture_count) = tokens(pattern);
    let mut capture_ranges = vec![None; capture_count];
    let mut choices = Vec::new();
    let mut value_position = 0_usize;
    let mut pattern_position = 0_usize;
    let mut steps = 0_usize;

    loop {
        steps = steps.saturating_add(1);
        if steps > MAX_GLOB_STEPS {
            return Err(GlobError::WorkLimit);
        }
        match tokens.get(pattern_position).copied() {
            None if value_position == value.len() => {
                return Ok(materialize(value, capture_ranges));
            }
            None => {
                let Some(next_pattern) = retry(
                    &mut choices,
                    &mut capture_ranges,
                    &mut value_position,
                    value.len(),
                    &mut steps,
                )?
                else {
                    return Ok(None);
                };
                pattern_position = next_pattern;
            }
            Some(GlobToken::Many(capture)) => {
                let Some(()) =
                    record_capture(&mut capture_ranges, capture, value_position..value_position)
                else {
                    return Ok(None);
                };
                choices.push(StarChoice {
                    next_pattern: pattern_position.saturating_add(1),
                    capture,
                    start: value_position,
                    next_end: value_position.saturating_add(1),
                });
                pattern_position = pattern_position.saturating_add(1);
            }
            Some(GlobToken::One(capture)) => match value.get(value_position) {
                Some(_) => {
                    let end = value_position.saturating_add(1);
                    let Some(()) =
                        record_capture(&mut capture_ranges, capture, value_position..end)
                    else {
                        return Ok(None);
                    };
                    value_position = end;
                    pattern_position = pattern_position.saturating_add(1);
                }
                None => {
                    let Some(next_pattern) = retry(
                        &mut choices,
                        &mut capture_ranges,
                        &mut value_position,
                        value.len(),
                        &mut steps,
                    )?
                    else {
                        return Ok(None);
                    };
                    pattern_position = next_pattern;
                }
            },
            Some(GlobToken::Literal(expected)) => match value.get(value_position) {
                Some(actual) if equal_octet(*actual, expected, comparator) => {
                    value_position = value_position.saturating_add(1);
                    pattern_position = pattern_position.saturating_add(1);
                }
                _ => {
                    let Some(next_pattern) = retry(
                        &mut choices,
                        &mut capture_ranges,
                        &mut value_position,
                        value.len(),
                        &mut steps,
                    )?
                    else {
                        return Ok(None);
                    };
                    pattern_position = next_pattern;
                }
            },
        }
    }
}

/// Увеличивает ближайшую звезду; исчерпанная звезда передаёт откат предыдущей.
fn retry(
    choices: &mut Vec<StarChoice>,
    captures: &mut [Option<Range<usize>>],
    value_position: &mut usize,
    value_length: usize,
    steps: &mut usize,
) -> Result<Option<usize>, GlobError> {
    loop {
        *steps = steps.saturating_add(1);
        if *steps > MAX_GLOB_STEPS {
            return Err(GlobError::WorkLimit);
        }
        let Some(choice) = choices.last_mut() else {
            return Ok(None);
        };
        if choice.next_end > value_length {
            let _ = choices.pop();
            continue;
        }
        let end = choice.next_end;
        choice.next_end = end.saturating_add(1);
        *value_position = end;
        clear_captures_after(captures, choice.capture);
        let Some(()) = record_capture(captures, choice.capture, choice.start..end) else {
            return Ok(None);
        };
        return Ok(Some(choice.next_pattern));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GlobError {
    WorkLimit,
}

/// Очищает captures, инвалидированные изменением длины более ранней `*`.
fn clear_captures_after(captures: &mut [Option<Range<usize>>], capture: usize) {
    let Some(start) = capture.checked_add(1) else {
        return;
    };
    for position in start..captures.len() {
        if let Some(slot) = captures.get_mut(position) {
            *slot = None;
        }
    }
}

/// Записывает capture без прямого индексирования коллекции.
fn record_capture(
    captures: &mut [Option<Range<usize>>],
    index: usize,
    range: Range<usize>,
) -> Option<()> {
    match captures.get_mut(index) {
        Some(slot) => {
            *slot = Some(range);
            Some(())
        }
        None => None,
    }
}

/// Копирует byte ranges; они всегда созданы на границах входных octets.
fn materialize(value: &[u8], ranges: Vec<Option<Range<usize>>>) -> Option<Vec<Vec<u8>>> {
    ranges
        .into_iter()
        .map(|range| {
            let range = range?;
            Some(value.get(range)?.to_vec())
        })
        .collect()
}

/// Сравнивает один octet с учётом выбранной Sieve comparator policy.
fn equal_octet(value: u8, expected: u8, comparator: &Comparator) -> bool {
    match comparator {
        Comparator::AsciiCasemap => value.eq_ignore_ascii_case(&expected),
        Comparator::Octet => value == expected,
        Comparator::Variable(_) => false,
    }
}

/// Нумерует только wildcard-ы, которые не экранированы обратной косой чертой.
fn tokens(pattern: &[u8]) -> (Vec<GlobToken>, usize) {
    let mut bytes = pattern.iter().copied().peekable();
    let mut tokens = Vec::new();
    let mut capture_count = 0_usize;
    while let Some(byte) = bytes.next() {
        let token = match (byte, bytes.peek().copied()) {
            (b'\\', Some(b'*' | b'?')) => match bytes.next() {
                Some(value) => GlobToken::Literal(value),
                None => GlobToken::Literal(byte),
            },
            (b'*', _) => numbered(GlobKind::Many, &mut capture_count),
            (b'?', _) => numbered(GlobKind::One, &mut capture_count),
            _ => GlobToken::Literal(byte),
        };
        tokens.push(token);
    }
    (tokens, capture_count)
}

/// Выдаёт wildcard-у его позицию в capture result.
fn numbered(kind: GlobKind, count: &mut usize) -> GlobToken {
    let capture = *count;
    *count = count.saturating_add(1);
    match kind {
        GlobKind::One => GlobToken::One(capture),
        GlobKind::Many => GlobToken::Many(capture),
    }
}

struct StarChoice {
    next_pattern: usize,
    capture: usize,
    start: usize,
    next_end: usize,
}

#[derive(Clone, Copy)]
enum GlobToken {
    Literal(u8),
    One(usize),
    Many(usize),
}

#[derive(Clone, Copy)]
enum GlobKind {
    One,
    Many,
}

#[cfg(test)]
#[path = "glob_tests.rs"]
mod tests;
