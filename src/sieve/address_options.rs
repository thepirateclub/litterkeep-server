//! Разбор tagged arguments Sieve address-test до позиционных string-list.

use super::ast::{AddressPart, Comparator, MatchType};
use super::keywords::AddressTag;
use super::lexer::Token;
use super::parser_values::one_string;
use std::collections::VecDeque;

/// Разбирает ADDRESS-PART, MATCH-TYPE и COMPARATOR в любом допустимом порядке.
pub(super) fn parse(
    input: &mut VecDeque<Token>,
) -> Result<(AddressPart, MatchType, Comparator), &'static str> {
    let mut part = None;
    let mut match_type = None;
    let mut selected_comparator = None;
    while let Some(Token::Atom(tag)) = input.front() {
        let option = match AddressTag::parse(tag) {
            AddressTag::All => AddressOption::Part(AddressPart::All),
            AddressTag::LocalPart => AddressOption::Part(AddressPart::LocalPart),
            AddressTag::Domain => AddressOption::Part(AddressPart::Domain),
            AddressTag::User => AddressOption::Part(AddressPart::User),
            AddressTag::Detail => AddressOption::Part(AddressPart::Detail),
            AddressTag::Is => AddressOption::Match(MatchType::Is),
            AddressTag::Contains => AddressOption::Match(MatchType::Contains),
            AddressTag::Matches => AddressOption::Match(MatchType::Matches),
            AddressTag::Comparator => AddressOption::Comparator,
            AddressTag::Unknown => match tag.starts_with(':') {
                true => return Err("unsupported sieve address tag"),
                false => break,
            },
        };
        let _ = input.pop_front();
        match option {
            AddressOption::Part(value) => set_once(&mut part, value)?,
            AddressOption::Match(value) => set_once(&mut match_type, value)?,
            AddressOption::Comparator => {
                let value = comparator(&one_string(input)?)?;
                set_once(&mut selected_comparator, value)?;
            }
        }
    }
    Ok((
        part.unwrap_or(AddressPart::All),
        match_type.unwrap_or(MatchType::Is),
        selected_comparator.unwrap_or(Comparator::AsciiCasemap),
    ))
}

/// Типизированное представление одного address-test argument.
enum AddressOption {
    Part(AddressPart),
    Match(MatchType),
    Comparator,
}

/// Не допускает неоднозначную повторную передачу одного optional argument.
fn set_once<T>(slot: &mut Option<T>, value: T) -> Result<(), &'static str> {
    match slot.replace(value) {
        Some(_) => Err("duplicate sieve address option"),
        None => Ok(()),
    }
}

/// Сопоставляет wire-имя comparator с поддерживаемым enum.
fn comparator(value: &str) -> Result<Comparator, &'static str> {
    match super::keywords::ComparatorWord::parse(value) {
        super::keywords::ComparatorWord::AsciiCasemap => Ok(Comparator::AsciiCasemap),
        super::keywords::ComparatorWord::Octet => Ok(Comparator::Octet),
        super::keywords::ComparatorWord::Unknown if value.contains("${") => {
            Ok(Comparator::Variable(value.to_owned()))
        }
        _ => Err("unsupported sieve comparator"),
    }
}
