//! Преобразование IMAP flag atoms в общую типизированную модель.

use super::sequence_set::SequenceSet;
use super::tokenizer::Token;
use super::{Addressing, Command};
use crate::mail_flags::{FlagMutation, KeywordFlag, MessageFlags, StoredFlags, SystemFlag};
use std::collections::BTreeSet;

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

/// Разделяет зарегистрированные системные флаги от расширяемых keywords.
pub fn parse_stored_set(values: impl IntoIterator<Item = String>) -> Option<StoredFlags> {
    values
        .into_iter()
        .try_fold(StoredFlags::default(), |mut stored, value| {
            let normalized = value.to_ascii_uppercase();
            match system_flag(&normalized) {
                Some(flag) => stored.system = add(stored.system, flag),
                None => match normalized.starts_with('\\') {
                    true => return None,
                    false => {
                        stored.keywords.insert(KeywordFlag::parse(&value)?);
                    }
                },
            }
            Some(stored)
        })
}

fn system_flag(value: &str) -> Option<SystemFlag> {
    match value {
        "\\ANSWERED" => Some(SystemFlag::Answered),
        "\\FLAGGED" => Some(SystemFlag::Flagged),
        "\\DELETED" => Some(SystemFlag::Deleted),
        "\\SEEN" => Some(SystemFlag::Seen),
        "\\DRAFT" => Some(SystemFlag::Draft),
        _ => None,
    }
}

fn add(flags: MessageFlags, flag: SystemFlag) -> MessageFlags {
    MessageFlags::from_flags([flag]).mutate(crate::mail_flags::FlagMutation::Add, flags)
}

/// Преобразует системные флаги и keywords в один STORE операнд.
pub fn parse_store_set(
    values: impl IntoIterator<Item = String>,
) -> Option<(MessageFlags, BTreeSet<KeywordFlag>)> {
    let flags = parse_stored_set(values)?;
    Some((flags.system, flags.keywords))
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
        Token::Quoted(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    });
    match parse_store_set(values) {
        Some((flags, keywords)) => Command::Store {
            addressing,
            sequence_set,
            mutation: mode.mutation,
            silent: mode.silent,
            flags,
            keywords,
        },
        None => Command::Unsupported,
    }
}

fn next_text(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    tokens.find_map(|token| match token {
        Token::Text(value) | Token::Quoted(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    })
}

/// Кодирует флаги в каноническом порядке без сохранения строк в store.
pub fn encode(flags: MessageFlags) -> String {
    encode_with_recent(flags, false)
}

/// Добавляет session-local `\\Recent`, не записывая её в persistent bitset.
pub fn encode_with_recent(flags: MessageFlags, recent: bool) -> String {
    let ordered = [
        (SystemFlag::Answered, "\\Answered"),
        (SystemFlag::Flagged, "\\Flagged"),
        (SystemFlag::Deleted, "\\Deleted"),
        (SystemFlag::Seen, "\\Seen"),
        (SystemFlag::Draft, "\\Draft"),
    ];
    let mut values = ordered
        .into_iter()
        .filter_map(|(flag, name)| flags.contains(flag).then_some(name))
        .collect::<Vec<_>>();
    if recent {
        values.push("\\Recent");
    }
    values.join(" ")
}

/// Кодирует все сохранённые флаги, добавляя только session-local Recent.
pub fn encode_stored(
    flags: MessageFlags,
    keywords: &BTreeSet<KeywordFlag>,
    recent: bool,
) -> String {
    let mut values = encode_with_recent(flags, recent)
        .split_ascii_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    values.extend(keywords.iter().map(|keyword| keyword.as_str().to_owned()));
    values.join(" ")
}
