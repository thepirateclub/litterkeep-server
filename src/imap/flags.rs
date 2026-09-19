//! Преобразование IMAP flag atoms в общую типизированную модель.

use super::sequence_set::SequenceSet;
use super::tokenizer::Token;
use super::{Addressing, Command};
use crate::mail_flags::{FlagMutation, MessageFlags, SystemFlag};

/// Режим STORE, включая подавление untagged FETCH ответа.
pub struct StoreMode {
    pub mutation: FlagMutation,
    pub silent: bool,
}

/// Единственная граница строкового сравнения STORE data item.
pub fn parse_mode(value: &str) -> Option<StoreMode> {
    match value.to_ascii_uppercase().as_str() {
        "FLAGS" => Some(StoreMode {
            mutation: FlagMutation::Replace,
            silent: false,
        }),
        "FLAGS.SILENT" => Some(StoreMode {
            mutation: FlagMutation::Replace,
            silent: true,
        }),
        "+FLAGS" => Some(StoreMode {
            mutation: FlagMutation::Add,
            silent: false,
        }),
        "+FLAGS.SILENT" => Some(StoreMode {
            mutation: FlagMutation::Add,
            silent: true,
        }),
        "-FLAGS" => Some(StoreMode {
            mutation: FlagMutation::Remove,
            silent: false,
        }),
        "-FLAGS.SILENT" => Some(StoreMode {
            mutation: FlagMutation::Remove,
            silent: true,
        }),
        _ => None,
    }
}

/// Преобразует ограниченный список системных flag atoms в bitset.
pub fn parse_set(values: impl IntoIterator<Item = String>) -> Option<MessageFlags> {
    values
        .into_iter()
        .map(|value| match value.to_ascii_uppercase().as_str() {
            "\\ANSWERED" => Some(SystemFlag::Answered),
            "\\FLAGGED" => Some(SystemFlag::Flagged),
            "\\DELETED" => Some(SystemFlag::Deleted),
            "\\SEEN" => Some(SystemFlag::Seen),
            "\\DRAFT" => Some(SystemFlag::Draft),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .map(MessageFlags::from_flags)
}

/// Разбирает аргументы STORE после уже типизированного пространства адресации.
pub fn parse_store(tokens: &mut impl Iterator<Item = Token>, addressing: Addressing) -> Command {
    let Some(sequence_set) = next_text(tokens).and_then(|value| SequenceSet::parse(&value).ok())
    else {
        return Command::Unsupported;
    };
    let Some(mode) = next_text(tokens).and_then(|value| parse_mode(&value)) else {
        return Command::Unsupported;
    };
    let values = tokens.filter_map(|token| match token {
        Token::Text(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    });
    match parse_set(values) {
        Some(flags) => Command::Store {
            addressing,
            sequence_set,
            mutation: mode.mutation,
            silent: mode.silent,
            flags,
        },
        None => Command::Unsupported,
    }
}

fn next_text(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    tokens.find_map(|token| match token {
        Token::Text(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    })
}

/// Кодирует флаги в каноническом порядке без сохранения строк в store.
pub fn encode(flags: MessageFlags) -> String {
    let ordered = [
        (SystemFlag::Answered, "\\Answered"),
        (SystemFlag::Flagged, "\\Flagged"),
        (SystemFlag::Deleted, "\\Deleted"),
        (SystemFlag::Seen, "\\Seen"),
        (SystemFlag::Draft, "\\Draft"),
    ];
    ordered
        .into_iter()
        .filter_map(|(flag, name)| flags.contains(flag).then_some(name))
        .collect::<Vec<_>>()
        .join(" ")
}
