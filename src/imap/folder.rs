//! Типизированная грамматика начального набора команд управления папками.

use super::tokenizer::Token;
use super::{Command, MailboxName};

/// Ограниченный и явно поддерживаемый LIST wildcard profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListPattern {
    All,
    OneLevel,
    Exact(String),
    Wildcard(String),
}

/// Операция нужна completion-слою для точного tagged-ответа.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mutation {
    Delete,
    Rename,
}

pub fn parse_create(mut tokens: impl Iterator<Item = Token>) -> Command {
    match (next_text(&mut tokens), tokens.next()) {
        (Some(name), None) => mailbox(name)
            .map(|mailbox| Command::Create { mailbox })
            .unwrap_or(Command::Unsupported),
        _ => Command::Unsupported,
    }
}

pub fn parse_delete(mut tokens: impl Iterator<Item = Token>) -> Command {
    match (next_text(&mut tokens), tokens.next()) {
        (Some(name), None) => mailbox(name)
            .map(|mailbox| Command::Delete { mailbox })
            .unwrap_or(Command::Unsupported),
        _ => Command::Unsupported,
    }
}

pub fn parse_rename(mut tokens: impl Iterator<Item = Token>) -> Command {
    match (
        next_text(&mut tokens),
        next_text(&mut tokens),
        tokens.next(),
    ) {
        (Some(source), Some(destination), None) => match (mailbox(source), mailbox(destination)) {
            (Some(source), Some(destination)) => Command::Rename {
                source,
                destination,
            },
            _ => Command::Unsupported,
        },
        _ => Command::Unsupported,
    }
}

pub fn parse_subscribe(mut tokens: impl Iterator<Item = Token>, subscribed: bool) -> Command {
    let parsed = match (next_text(&mut tokens), tokens.next()) {
        (Some(name), None) => mailbox(name),
        _ => None,
    };
    match (parsed, subscribed) {
        (Some(mailbox), true) => Command::Subscribe { mailbox },
        (Some(mailbox), false) => Command::Unsubscribe { mailbox },
        (None, _) => Command::Unsupported,
    }
}

pub fn parse_list(mut tokens: impl Iterator<Item = Token>) -> Command {
    match tokens.next() {
        Some(Token::ListStart) => match (tokens.next(), tokens.next()) {
            (Some(Token::Text(option)), Some(Token::ListEnd))
                if option.eq_ignore_ascii_case("SPECIAL-USE") =>
            {
                parse_pattern(&mut tokens)
                    .map(|pattern| Command::ListSpecial { pattern })
                    .unwrap_or(Command::Unsupported)
            }
            _ => Command::Unsupported,
        },
        Some(first) => {
            let mut restored = std::iter::once(first).chain(tokens);
            parse_pattern(&mut restored)
                .map(|pattern| Command::List { pattern })
                .unwrap_or(Command::Unsupported)
        }
        None => Command::Unsupported,
    }
}

fn parse_pattern(tokens: &mut impl Iterator<Item = Token>) -> Option<ListPattern> {
    match (next_text(tokens), next_text(tokens), tokens.next()) {
        (Some(reference), Some(pattern), None) if reference.is_empty() => {
            Some(match pattern.as_str() {
                "*" => ListPattern::All,
                "%" => ListPattern::OneLevel,
                _ => list_pattern(super::modified_utf7::decode(&pattern).ok()?),
            })
        }
        _ => None,
    }
}

fn list_pattern(pattern: String) -> ListPattern {
    match (pattern.contains('*'), pattern.contains('%')) {
        (false, false) => ListPattern::Exact(pattern),
        _ => ListPattern::Wildcard(pattern),
    }
}

pub fn parse_lsub(mut tokens: impl Iterator<Item = Token>) -> Command {
    match parse_pattern(&mut tokens) {
        Some(pattern) => Command::Lsub { pattern },
        None => Command::Unsupported,
    }
}

pub(super) fn mailbox(value: String) -> Option<MailboxName> {
    let decoded = super::modified_utf7::decode(&value).ok()?;
    match decoded.to_ascii_uppercase().as_str() {
        "INBOX" => Some(MailboxName::Inbox),
        _ => Some(MailboxName::Named(decoded)),
    }
}

fn next_text(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    match tokens.next()? {
        Token::Text(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    }
}
