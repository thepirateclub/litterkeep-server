//! Преобразование IMAP flag atoms в общую типизированную модель.

use super::sequence_set::MessageSet;
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
    let Some(sequence_set) = tokens
        .next()
        .and_then(super::token_values::atom_value)
        .and_then(|value| MessageSet::parse(&value).ok())
    else {
        return Command::Unsupported;
    };
    let Some(mode) = tokens
        .next()
        .and_then(super::token_values::atom_value)
        .and_then(|value| parse_mode(&value))
    else {
        return Command::Unsupported;
    };
    match parse_store_flags(tokens) {
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

/// Разбирает parenthesized либо unparenthesized STORE flags без потери границ.
fn parse_store_flags(
    tokens: &mut impl Iterator<Item = Token>,
) -> Option<(MessageFlags, BTreeSet<KeywordFlag>)> {
    match tokens.next()? {
        Token::ListStart => {
            let values = parse_parenthesized_flag_values(tokens)?;
            match tokens.next() {
                None => parse_store_set(values),
                Some(_) => None,
            }
        }
        token => {
            let mut values = vec![flag_value(token)?];
            for token in tokens {
                values.push(flag_value(token)?);
            }
            parse_store_set(values)
        }
    }
}

/// APPEND flag-list и STORE flag-list требуют ровно одну закрытую скобочную пару.
pub(super) fn parse_parenthesized_flag_list(
    tokens: &mut impl Iterator<Item = Token>,
) -> Option<StoredFlags> {
    if !matches!(tokens.next(), Some(Token::ListStart)) {
        return None;
    }
    parse_stored_set(parse_parenthesized_flag_values(tokens)?)
}

fn parse_parenthesized_flag_values(
    tokens: &mut impl Iterator<Item = Token>,
) -> Option<Vec<String>> {
    let mut values = Vec::new();
    loop {
        match tokens.next()? {
            Token::Text(value) => values.push(flag_value(Token::Text(value))?),
            Token::ListEnd => return Some(values),
            Token::Quoted(_) | Token::Literal(_) | Token::ListStart => return None,
        }
    }
}

fn flag_value(token: Token) -> Option<String> {
    match token {
        Token::Text(value) if supported_flag(&value) => Some(value),
        Token::Text(_)
        | Token::Quoted(_)
        | Token::Literal(_)
        | Token::ListStart
        | Token::ListEnd => None,
    }
}

fn supported_flag(value: &str) -> bool {
    match system_flag(&value.to_ascii_uppercase()) {
        Some(_) => true,
        None => KeywordFlag::parse(value).is_some(),
    }
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
