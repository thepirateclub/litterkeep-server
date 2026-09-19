//! Типизированная грамматика начального набора команд управления папками.

use super::tokenizer::Token;
use super::{Command, MailboxName};

/// Ограниченный и явно поддерживаемый LIST wildcard profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListPattern {
    All,
    OneLevel,
    Exact(String),
}

/// Операция нужна completion-слою для точного tagged-ответа.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mutation {
    Delete,
    Rename,
}

pub fn parse_create(mut tokens: impl Iterator<Item = Token>) -> Command {
    match (next_text(&mut tokens), tokens.next()) {
        (Some(name), None) => Command::Create {
            mailbox: mailbox(name),
        },
        _ => Command::Unsupported,
    }
}

pub fn parse_delete(mut tokens: impl Iterator<Item = Token>) -> Command {
    match (next_text(&mut tokens), tokens.next()) {
        (Some(name), None) => Command::Delete {
            mailbox: mailbox(name),
        },
        _ => Command::Unsupported,
    }
}

pub fn parse_rename(mut tokens: impl Iterator<Item = Token>) -> Command {
    match (
        next_text(&mut tokens),
        next_text(&mut tokens),
        tokens.next(),
    ) {
        (Some(source), Some(destination), None) => Command::Rename {
            source: mailbox(source),
            destination: mailbox(destination),
        },
        _ => Command::Unsupported,
    }
}

pub fn parse_list(mut tokens: impl Iterator<Item = Token>) -> Command {
    let values = (
        next_text(&mut tokens),
        next_text(&mut tokens),
        tokens.next(),
    );
    match values {
        (Some(reference), Some(pattern), None) if reference.is_empty() => {
            let pattern = match pattern.as_str() {
                "*" => ListPattern::All,
                "%" => ListPattern::OneLevel,
                _ => ListPattern::Exact(pattern),
            };
            Command::List { pattern }
        }
        _ => Command::Unsupported,
    }
}

fn mailbox(value: String) -> MailboxName {
    match value.to_ascii_uppercase().as_str() {
        "INBOX" => MailboxName::Inbox,
        _ => MailboxName::Named(value),
    }
}

fn next_text(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    match tokens.next()? {
        Token::Text(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    }
}
