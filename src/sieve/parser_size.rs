//! Разбор RFC 5228 `size` test-а и двоичных size quantifier-ов.

use super::super::ast::{Condition, SizeRelation};
use super::super::keywords::SizeTag;
use super::super::lexer::Token;
use super::super::parser_values::atom;
use std::collections::VecDeque;

pub(super) fn parse(input: &mut VecDeque<Token>) -> Result<Condition, &'static str> {
    let relation = match SizeTag::parse(&atom(input)?) {
        SizeTag::Over => SizeRelation::Over,
        SizeTag::Under => SizeRelation::Under,
        _ => return Err("sieve size relation expected"),
    };
    Ok(Condition::Size {
        relation,
        limit: size_limit(&atom(input)?)?,
    })
}

/// `K`, `M`, `G` переводятся в октеты с обязательной проверкой переполнения.
fn size_limit(value: &str) -> Result<usize, &'static str> {
    let (number, factor) = match value.chars().last() {
        Some('K' | 'k') => (value.get(..value.len().saturating_sub(1)), 1_024_usize),
        Some('M' | 'm') => (value.get(..value.len().saturating_sub(1)), 1_048_576_usize),
        Some('G' | 'g') => (
            value.get(..value.len().saturating_sub(1)),
            1_073_741_824_usize,
        ),
        Some(_) => (Some(value), 1),
        None => (None, 1),
    };
    number
        .ok_or("invalid sieve size")?
        .parse::<usize>()
        .map_err(|_| "invalid sieve size")?
        .checked_mul(factor)
        .ok_or("sieve size overflow")
}
