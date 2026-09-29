//! Grammar Sieve delivery actions и их optional tagged arguments.

use super::super::ast::Action;
use super::super::keywords::CopyTag;
use super::super::lexer::Token;
use super::super::parser_values::one_octet_string;
use std::collections::VecDeque;

#[path = "parser_vacation.rs"]
mod vacation_action;
#[path = "parser_variables.rs"]
mod variables_action;

/// Разбирает optional RFC 3894 `:copy` перед обязательным mailbox string.
pub(super) fn fileinto(input: &mut VecDeque<Token>) -> Result<Action, &'static str> {
    match input.front() {
        Some(Token::Atom(tag)) if CopyTag::parse(tag) == CopyTag::Copy => {
            let _ = input.pop_front();
            Ok(Action::FileIntoCopy(one_octet_string(input)?))
        }
        _ => Ok(Action::FileInto(one_octet_string(input)?)),
    }
}

/// Разбирает RFC 3894 `redirect :copy`, сохраняя implicit keep.
pub(super) fn redirect(input: &mut VecDeque<Token>) -> Result<Action, &'static str> {
    match input.front() {
        Some(Token::Atom(tag)) if CopyTag::parse(tag) == CopyTag::Copy => {
            let _ = input.pop_front();
            Ok(Action::RedirectCopy(one_octet_string(input)?))
        }
        _ => Ok(Action::Redirect(one_octet_string(input)?)),
    }
}

/// Делегирует разбор RFC 5230 опций отдельной grammar-функции.
pub(super) fn vacation(input: &mut VecDeque<Token>) -> Result<Action, &'static str> {
    vacation_action::parse(input)
}

/// Делегирует grammar RFC 5229 с shared modifier enum.
pub(super) fn set(input: &mut VecDeque<Token>) -> Result<Action, &'static str> {
    variables_action::parse_set(input)
}
