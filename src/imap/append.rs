//! Разбор опциональных флагов, INTERNALDATE и literal команды APPEND.

use super::flags;
use super::tokenizer::Token;
use super::{Command, MailboxName};
use crate::mail_date::MailDate;
use crate::mail_flags::StoredFlags;

/// Разбирает APPEND слева направо, не угадывая назначение уже разобранных полей.
pub fn parse(tokens: impl Iterator<Item = Token>) -> Command {
    let mut tokens = tokens.peekable();
    let Some(mailbox) = next_text(&mut tokens).and_then(mailbox) else {
        return Command::Unsupported;
    };
    let stored_flags = match tokens.peek() {
        Some(Token::ListStart) => match flags::parse_parenthesized_flag_list(&mut tokens) {
            Some(flags) => flags,
            None => return Command::Unsupported,
        },
        Some(Token::Text(_)) | Some(Token::Quoted(_)) | Some(Token::ListEnd) | None => {
            StoredFlags::default()
        }
    };
    let Some((internal_date, literal_size)) = date_and_literal(&mut tokens) else {
        return Command::Unsupported;
    };
    match tokens.next() {
        None => Command::Append {
            mailbox,
            literal_size,
            flags: stored_flags.system,
            keywords: stored_flags.keywords,
            internal_date,
        },
        Some(_) => Command::Unsupported,
    }
}

fn mailbox(value: String) -> Option<MailboxName> {
    super::folder::mailbox(value)
}

fn next_text(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    match tokens.next()? {
        Token::Text(value) | Token::Quoted(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    }
}

fn literal_size(value: &str) -> Option<usize> {
    value
        .strip_prefix('{')?
        .strip_suffix('}')?
        .parse::<usize>()
        .ok()
}

/// Сохраняет синтаксическую форму RFC: INTERNALDATE обязан быть quoted-string.
fn date_and_literal(tokens: &mut impl Iterator<Item = Token>) -> Option<(Option<MailDate>, usize)> {
    match tokens.next()? {
        Token::Text(value) => Some((None, literal_size(&value)?)),
        Token::Quoted(value) => {
            let date = MailDate::parse(&value)?;
            match tokens.next()? {
                Token::Text(value) => Some((Some(date), literal_size(&value)?)),
                Token::Quoted(_) | Token::ListStart | Token::ListEnd => None,
            }
        }
        Token::ListStart | Token::ListEnd => None,
    }
}
