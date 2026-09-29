//! Грамматика RFC 5229 `string` test отделена от базового test dispatcher.

use super::super::ast::{Condition, StringMatch};
use super::super::lexer::Token;
use super::super::parser_values::string_list;
use super::match_options;
use std::collections::VecDeque;

/// Разбирает пары string-list, сохраняя пробелы исходных строк без изменений.
pub(super) fn parse(input: &mut VecDeque<Token>) -> Result<Condition, &'static str> {
    let (match_type, comparator) = match_options(input)?;
    Ok(Condition::String {
        sources: string_list(input)?,
        matcher: StringMatch {
            match_type,
            comparator,
            keys: string_list(input)?,
        },
    })
}
