//! Grammar Sieve delivery actions и их optional tagged arguments.

use super::super::ast::Action;
use super::super::lexer::Token;
use super::super::parser_values::one_string;
use std::collections::VecDeque;

#[path = "parser_vacation.rs"]
mod vacation_action;

/// Разбирает optional RFC 3894 `:copy` перед обязательным mailbox string.
pub(super) fn fileinto(input: &mut VecDeque<Token>) -> Result<Action, &'static str> {
    match input.front() {
        Some(Token::Atom(tag)) if tag == ":copy" => {
            let _ = input.pop_front();
            Ok(Action::FileIntoCopy(one_string(input)?))
        }
        _ => Ok(Action::FileInto(one_string(input)?)),
    }
}

/// Разбирает RFC 3894 `redirect :copy`, сохраняя implicit keep.
pub(super) fn redirect(input: &mut VecDeque<Token>) -> Result<Action, &'static str> {
    match input.front() {
        Some(Token::Atom(tag)) if tag == ":copy" => {
            let _ = input.pop_front();
            Ok(Action::RedirectCopy(one_string(input)?))
        }
        _ => Ok(Action::Redirect(one_string(input)?)),
    }
}

/// Делегирует разбор RFC 5230 опций отдельной grammar-функции.
pub(super) fn vacation(input: &mut VecDeque<Token>) -> Result<Action, &'static str> {
    vacation_action::parse(input)
}
