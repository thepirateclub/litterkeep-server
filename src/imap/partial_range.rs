//! Типизированный диапазон постраничного SEARCH/FETCH по RFC 9394.

use std::ops::Range;

/// Два направления нумерации результата: от начала или от самого нового.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Direction {
    Oldest { first: u64, last: u64 },
    Newest { first: u64, last: u64 },
}

/// PARTIAL range хранит исходную форму для ответа ESEARCH.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartialRange {
    wire: String,
    direction: Direction,
}

impl PartialRange {
    /// Разбирает только пары ненулевых номеров, разрешённые RFC 9394.
    pub fn parse(value: &str) -> Option<Self> {
        let (left, right) = value.split_once(':')?;
        if right.contains(':') {
            return None;
        }
        let direction = match (left.strip_prefix('-'), right.strip_prefix('-')) {
            (Some(first), Some(last)) => Direction::Newest {
                first: number(first)?,
                last: number(last)?,
            },
            (None, None) => Direction::Oldest {
                first: number(left)?,
                last: number(right)?,
            },
            _ => return None,
        };
        Some(Self {
            wire: value.to_owned(),
            direction,
        })
    }

    /// Возвращает исходный диапазон для PARTIAL response data.
    pub fn wire(&self) -> &str {
        &self.wire
    }

    /// Преобразует 1-based диапазон в clamped полуоткрытые позиции результата.
    pub fn positions(&self, result_count: usize) -> Range<usize> {
        match self.direction {
            Direction::Oldest { first, last } => {
                let lower = first.min(last).saturating_sub(1);
                let upper = first.max(last);
                let start = clamp(lower, result_count);
                let end = clamp(upper, result_count);
                start.min(end)..end
            }
            Direction::Newest { first, last } => {
                let nearer = first.min(last);
                let farther = first.max(last);
                let lower = result_count.saturating_sub(to_usize(farther));
                let upper = result_count.saturating_sub(to_usize(nearer).saturating_sub(1));
                lower.min(upper)..upper
            }
        }
    }
}

fn number(value: &str) -> Option<u64> {
    match (value.is_empty(), value.starts_with('0')) {
        (false, false) => value.parse::<u64>().ok(),
        _ => None,
    }
}

fn to_usize(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn clamp(value: u64, maximum: usize) -> usize {
    to_usize(value).min(maximum)
}

#[cfg(test)]
#[path = "partial_range_tests.rs"]
mod tests;
