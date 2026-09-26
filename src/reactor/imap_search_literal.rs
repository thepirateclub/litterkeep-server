//! Ограниченный RFC synchronizing literal framing для IMAP-команд.

use crate::imap::command_verb::{Verb, parse_verb};
pub(super) use crate::imap::literal::Mode as LiteralMode;
use crate::imap::literal::{self, Marker as LiteralMarker};

#[path = "imap_search_literal_receiver.rs"]
mod receiver;
pub(in crate::reactor) use receiver::{Progress, Receiver};

/// Решение до выдачи continuation клиенту.
pub(super) enum Start {
    NotLiteral,
    Begin {
        receiver: Receiver,
        mode: LiteralMode,
    },
    Reject {
        tag: String,
        reason: &'static str,
        terminate: bool,
    },
}

const MAX_IMAP_LITERAL: usize = 64 * 1024;
const MAX_NON_SYNCHRONIZING_LITERAL: usize = 4 * 1024;

/// Семейство команды определяет, допустим ли literal в последнем аргументе.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiteralCommand {
    Login,
    Search,
    Mailbox,
}

/// Ищет sync-literal в строковых и mailbox аргументах поддержанных команд.
pub(super) fn start(line: &[u8]) -> Start {
    let Ok(command) = std::str::from_utf8(line) else {
        return Start::NotLiteral;
    };
    let Some(command) = command.strip_suffix("\r\n") else {
        return Start::NotLiteral;
    };
    let Some((prefix, tag, marker)) = initial_marker(command) else {
        return Start::NotLiteral;
    };
    let within_generic_limit = marker.size <= MAX_IMAP_LITERAL;
    let within_non_sync_limit = match marker.mode {
        LiteralMode::Synchronizing => true,
        LiteralMode::NonSynchronizing => marker.size <= MAX_NON_SYNCHRONIZING_LITERAL,
    };
    match (within_generic_limit, within_non_sync_limit) {
        (true, true) => match Receiver::new(prefix.as_bytes().to_vec(), tag.to_owned(), marker) {
            Some(receiver) => Start::Begin {
                receiver,
                mode: marker.mode,
            },
            None => Start::Reject {
                tag: tag.to_owned(),
                reason: "IMAP command exceeds limit",
                terminate: matches!(marker.mode, LiteralMode::NonSynchronizing),
            },
        },
        (false, _) | (true, false) => Start::Reject {
            tag: tag.to_owned(),
            reason: "IMAP literal exceeds configured limit",
            terminate: matches!(marker.mode, LiteralMode::NonSynchronizing),
        },
    }
}

fn initial_marker(command: &str) -> Option<(&str, &str, LiteralMarker)> {
    let boundary = command.rfind(|character: char| character.is_ascii_whitespace())?;
    let prefix_end = boundary.checked_add(1)?;
    let prefix = command.get(..prefix_end)?;
    let marker = command.get(boundary..)?.trim();
    let tag = literal_tag(prefix)?;
    Some((prefix, tag, literal::parse(marker)?))
}

fn literal_tag(prefix: &str) -> Option<&str> {
    let mut words = prefix.split_ascii_whitespace();
    let tag = words.next()?;
    let first = parse_verb(words.next()?);
    let _recognized_command = match first {
        Verb::Login => LiteralCommand::Login,
        Verb::Search => LiteralCommand::Search,
        Verb::Uid => match words.next().map(parse_verb) {
            Some(Verb::Search) => LiteralCommand::Search,
            Some(Verb::Copy | Verb::Move) => LiteralCommand::Mailbox,
            _ => return None,
        },
        Verb::Create
        | Verb::Delete
        | Verb::Rename
        | Verb::Subscribe
        | Verb::Unsubscribe
        | Verb::List
        | Verb::Lsub
        | Verb::Status
        | Verb::Copy
        | Verb::Move
        | Verb::Select
        | Verb::Examine => LiteralCommand::Mailbox,
        _ => return None,
    };
    Some(tag)
}

#[cfg(test)]
#[path = "imap_search_literal_tests.rs"]
mod tests;
