//! Glob сопоставление Sieve с byte-captures для RFC 5229 Variables.

use crate::sieve::ast::Comparator;
use std::ops::Range;

/// Сопоставляет полную строку и возвращает captures wildcard-ов в порядке шаблона.
pub(super) fn captures(
    value: &str,
    pattern: &str,
    comparator: &Comparator,
) -> Option<Vec<Vec<u8>>> {
    let value = value.as_bytes();
    let (tokens, capture_count) = tokens(pattern);
    let mut capture_ranges = vec![None; capture_count];
    let mut value_position = 0_usize;
    let mut pattern_position = 0_usize;
    let mut latest_star = None;

    while value.get(value_position).is_some() {
        match tokens.get(pattern_position).copied() {
            Some(GlobToken::Many(capture)) => {
                record_capture(&mut capture_ranges, capture, value_position..value_position)?;
                latest_star = Some(StarState {
                    pattern_position,
                    capture,
                    start: value_position,
                });
                pattern_position = pattern_position.saturating_add(1);
            }
            Some(GlobToken::One(capture)) => {
                record_capture(
                    &mut capture_ranges,
                    capture,
                    value_position..value_position.saturating_add(1),
                )?;
                value_position = value_position.saturating_add(1);
                pattern_position = pattern_position.saturating_add(1);
            }
            Some(GlobToken::Literal(expected))
                if value
                    .get(value_position)
                    .is_some_and(|actual| equal_octet(*actual, expected, comparator)) =>
            {
                value_position = value_position.saturating_add(1);
                pattern_position = pattern_position.saturating_add(1);
            }
            _ => {
                let star = latest_star?;
                value_position = value_position.saturating_add(1);
                record_capture(
                    &mut capture_ranges,
                    star.capture,
                    star.start..value_position,
                )?;
                pattern_position = star.pattern_position.saturating_add(1);
            }
        }
    }

    match finish_empty_suffix(
        &tokens,
        &mut capture_ranges,
        &mut pattern_position,
        value_position,
    ) {
        true if tokens.get(pattern_position).is_none() => materialize(value, capture_ranges),
        _ => None,
    }
}

/// Принимает оставшиеся `*`, каждый из которых совпадает с пустой строкой.
fn finish_empty_suffix(
    tokens: &[GlobToken],
    captures: &mut [Option<Range<usize>>],
    position: &mut usize,
    value_position: usize,
) -> bool {
    while let Some(token) = tokens.get(*position).copied() {
        match token {
            GlobToken::Many(capture) => {
                let Some(()) = record_capture(captures, capture, value_position..value_position)
                else {
                    return false;
                };
                *position = position.saturating_add(1);
            }
            GlobToken::Literal(_) | GlobToken::One(_) => return false,
        }
    }
    true
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

/// Копирует byte ranges в captures; диапазоны строятся только из input boundaries.
fn materialize(value: &[u8], ranges: Vec<Option<Range<usize>>>) -> Option<Vec<Vec<u8>>> {
    ranges
        .into_iter()
        .map(|range| {
            let range = range?;
            Some(value.get(range)?.to_vec())
        })
        .collect()
}

/// Сопоставляет одну octet с literal согласно выбранному comparator.
fn equal_octet(value: u8, expected: u8, comparator: &Comparator) -> bool {
    match comparator {
        Comparator::AsciiCasemap => value.eq_ignore_ascii_case(&expected),
        Comparator::Octet => value == expected,
    }
}

/// Нумерует только незаключённые в escape wildcard-ы.
fn tokens(pattern: &str) -> (Vec<GlobToken>, usize) {
    let mut bytes = pattern.as_bytes().iter().copied().peekable();
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

/// Выдаёт wildcard-у позицию в результате captures.
fn numbered(kind: GlobKind, count: &mut usize) -> GlobToken {
    let capture = *count;
    *count = count.saturating_add(1);
    match kind {
        GlobKind::One => GlobToken::One(capture),
        GlobKind::Many => GlobToken::Many(capture),
    }
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

#[derive(Clone, Copy)]
struct StarState {
    pattern_position: usize,
    capture: usize,
    start: usize,
}

#[cfg(test)]
#[path = "glob_tests.rs"]
mod tests;
