//! Разбор tagged arguments Sieve address-test до позиционных string-list.

use super::ast::{AddressPart, Comparator, MatchType};
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
        let option = match tag.as_str() {
            ":all" => AddressOption::Part(AddressPart::All),
            ":localpart" => AddressOption::Part(AddressPart::LocalPart),
            ":domain" => AddressOption::Part(AddressPart::Domain),
            ":user" => AddressOption::Part(AddressPart::User),
            ":detail" => AddressOption::Part(AddressPart::Detail),
            ":is" => AddressOption::Match(MatchType::Is),
            ":contains" => AddressOption::Match(MatchType::Contains),
            ":matches" => AddressOption::Match(MatchType::Matches),
            ":comparator" => AddressOption::Comparator,
            value if value.starts_with(':') => return Err("unsupported sieve address tag"),
            _ => break,
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
    match value {
        "i;ascii-casemap" => Ok(Comparator::AsciiCasemap),
        "i;octet" => Ok(Comparator::Octet),
        _ => Err("unsupported sieve comparator"),
    }
}
