//! Разбор опциональных флагов, INTERNALDATE и literal команды APPEND.

use super::flags;
use super::tokenizer::Token;
use super::{Command, MailboxName};
use crate::mail_date::MailDate;
use crate::mail_flags::StoredFlags;

/// Разбирает APPEND слева направо, не угадывая назначение уже разобранных полей.
pub fn parse(tokens: impl Iterator<Item = Token>) -> Command {
    parse_with_mode(tokens, super::tokenizer::StringMode::Legacy)
}

/// Разбирает APPEND mailbox в выбранном wire string mode.
pub(super) fn parse_with_mode(
    tokens: impl Iterator<Item = Token>,
    mode: super::tokenizer::StringMode,
) -> Command {
    let mut tokens = tokens.peekable();
    let Some(mailbox) = next_text(&mut tokens).and_then(|value| mailbox(value, mode)) else {
        return Command::Unsupported;
    };
    let stored_flags = match tokens.peek() {
        Some(Token::ListStart) => match flags::parse_parenthesized_flag_list(&mut tokens) {
            Some(flags) => flags,
            None => return Command::Unsupported,
        },
        Some(Token::Text(_))
        | Some(Token::Quoted(_))
        | Some(Token::Literal(_))
        | Some(Token::ListEnd)
        | None => StoredFlags::default(),
    };
    let Some((internal_date, marker)) = date_and_literal(&mut tokens) else {
        return Command::Unsupported;
    };
    match tokens.next() {
        None => Command::Append {
            mailbox,
            literal_size: marker.size,
            literal_mode: marker.mode,
            flags: stored_flags.system,
            keywords: stored_flags.keywords,
            internal_date,
        },
        Some(_) => Command::Unsupported,
    }
}

fn mailbox(value: String, mode: super::tokenizer::StringMode) -> Option<MailboxName> {
    super::folder::mailbox_with_mode(value, mode)
}

fn next_text(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    match tokens.next()? {
        Token::Text(value) | Token::Quoted(value) => Some(value),
        Token::Literal(value) => String::from_utf8(value).ok(),
        Token::ListStart | Token::ListEnd => None,
    }
}

/// Сохраняет синтаксическую форму RFC: INTERNALDATE обязан быть quoted-string.
fn date_and_literal(
    tokens: &mut impl Iterator<Item = Token>,
) -> Option<(Option<MailDate>, crate::imap::literal::Marker)> {
    match tokens.next()? {
        Token::Text(value) => Some((None, crate::imap::literal::parse(&value)?)),
        Token::Quoted(value) => {
            let date = MailDate::parse(&value)?;
            match tokens.next()? {
                Token::Text(value) => Some((Some(date), crate::imap::literal::parse(&value)?)),
                Token::Quoted(_) | Token::Literal(_) | Token::ListStart | Token::ListEnd => None,
            }
        }
        Token::Literal(_) | Token::ListStart | Token::ListEnd => None,
    }
}
