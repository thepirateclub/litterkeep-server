//! Разбор опциональных флагов, INTERNALDATE и literal команды APPEND.

use super::flags;
use super::tokenizer::Token;
use super::{Command, MailboxName};
use crate::mail_date::MailDate;
use crate::mail_flags::{KeywordFlag, MessageFlags, StoredFlags};
use std::collections::BTreeSet;

/// Одно сообщение и его независимые начальные IMAP-метаданные.
#[derive(Debug, Eq, PartialEq)]
pub struct AppendMessage {
    pub bytes: Vec<u8>,
    pub flags: MessageFlags,
    pub keywords: BTreeSet<KeywordFlag>,
    pub internal_date: Option<MailDate>,
}

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
    let mut messages = Vec::new();
    loop {
        let stored_flags = match parse_flags(&mut tokens) {
            Some(flags) => flags,
            None => return Command::Unsupported,
        };
        let Some(internal_date) = optional_date(&mut tokens) else {
            return Command::Unsupported;
        };
        match tokens.next() {
            Some(Token::Literal(bytes)) => messages.push(AppendMessage {
                bytes,
                flags: stored_flags.system,
                keywords: stored_flags.keywords,
                internal_date,
            }),
            Some(Token::Text(marker)) if messages.is_empty() => {
                return match (tokens.next(), crate::imap::literal::parse(&marker)) {
                    (None, Some(value)) => Command::AppendStart {
                        mailbox,
                        literal_size: value.size,
                        literal_mode: value.mode,
                        flags: stored_flags.system,
                        keywords: stored_flags.keywords,
                        internal_date,
                    },
                    _ => Command::Unsupported,
                };
            }
            Some(Token::Text(_))
            | Some(Token::Quoted(_))
            | Some(Token::ListStart)
            | Some(Token::ListEnd)
            | None => return Command::Unsupported,
        }
        if tokens.peek().is_none() {
            break;
        }
    }
    Command::Append { mailbox, messages }
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

fn parse_flags<I: Iterator<Item = Token>>(
    tokens: &mut std::iter::Peekable<I>,
) -> Option<StoredFlags> {
    match tokens.peek() {
        Some(Token::ListStart) => flags::parse_parenthesized_flag_list(tokens),
        Some(Token::Text(_))
        | Some(Token::Quoted(_))
        | Some(Token::Literal(_))
        | Some(Token::ListEnd)
        | None => Some(StoredFlags::default()),
    }
}

/// INTERNALDATE допустим только в quoted-string production.
fn optional_date<I: Iterator<Item = Token>>(
    tokens: &mut std::iter::Peekable<I>,
) -> Option<Option<MailDate>> {
    match tokens.peek() {
        Some(Token::Quoted(_)) => match tokens.next() {
            Some(Token::Quoted(value)) => MailDate::parse(&value).map(Some),
            _ => None,
        },
        Some(Token::Text(_))
        | Some(Token::Literal(_))
        | Some(Token::ListStart)
        | Some(Token::ListEnd)
        | None => Some(None),
    }
}
