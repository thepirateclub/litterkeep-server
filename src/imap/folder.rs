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
    let Some(name) = next_text(&mut tokens) else {
        return Command::Unsupported;
    };
    let special_use = match tokens.next() {
        None => Some(None),
        Some(Token::ListStart) => parse_use(&mut tokens).map(Some),
        _ => None,
    };
    match (mailbox(name), special_use) {
        (Some(mailbox), Some(special_use)) => Command::Create {
            mailbox,
            special_use,
        },
        _ => Command::Unsupported,
    }
}

fn parse_use(tokens: &mut impl Iterator<Item = Token>) -> Option<crate::store::SpecialUse> {
    match (
        tokens.next(),
        tokens.next(),
        tokens.next(),
        tokens.next(),
        tokens.next(),
    ) {
        (
            Some(Token::Text(use_name)),
            Some(Token::ListStart),
            Some(Token::Text(value)),
            Some(Token::ListEnd),
            Some(Token::ListEnd),
        ) if use_name.eq_ignore_ascii_case("USE") => special_use(&value),
        _ => None,
    }
}

fn special_use(value: &str) -> Option<crate::store::SpecialUse> {
    match value.to_ascii_uppercase().as_str() {
        "\\SENT" => Some(crate::store::SpecialUse::Sent),
        "\\DRAFTS" => Some(crate::store::SpecialUse::Drafts),
        "\\TRASH" => Some(crate::store::SpecialUse::Trash),
        "\\JUNK" => Some(crate::store::SpecialUse::Junk),
        "\\ARCHIVE" => Some(crate::store::SpecialUse::Archive),
        _ => None,
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
            parse_list_pattern(&mut restored)
                .map(|pattern| Command::List { pattern })
                .unwrap_or(Command::Unsupported)
        }
        None => Command::Unsupported,
    }
}

/// Принимает LIST RETURN (SPECIAL-USE): атрибуты уже вычисляются каталогом,
/// поэтому эта форма не меняет выборку, а требует их в каждом LIST-ответе.
fn parse_list_pattern(tokens: &mut impl Iterator<Item = Token>) -> Option<ListPattern> {
    let reference = next_text(tokens)?;
    let pattern = next_text(tokens)?;
    match tokens.next() {
        None if reference.is_empty() => list_pattern_value(pattern),
        Some(Token::Text(return_name)) if return_name.eq_ignore_ascii_case("RETURN") => {
            match (tokens.next(), tokens.next(), tokens.next(), tokens.next()) {
                (Some(Token::ListStart), Some(Token::Text(option)), Some(Token::ListEnd), None)
                    if reference.is_empty() && option.eq_ignore_ascii_case("SPECIAL-USE") =>
                {
                    list_pattern_value(pattern)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn list_pattern_value(pattern: String) -> Option<ListPattern> {
    Some(match pattern.as_str() {
        "*" => ListPattern::All,
        "%" => ListPattern::OneLevel,
        _ => list_pattern(super::modified_utf7::decode(&pattern).ok()?),
    })
}

fn parse_pattern(tokens: &mut impl Iterator<Item = Token>) -> Option<ListPattern> {
    match (next_text(tokens), next_text(tokens), tokens.next()) {
        (Some(reference), Some(pattern), None) if reference.is_empty() => {
            list_pattern_value(pattern)
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
