//! Типизированная грамматика начального набора команд управления папками.

use super::tokenizer::Token;
use super::{Command, MailboxName};

/// Ограниченный и явно поддерживаемый LIST wildcard profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListPattern {
    /// Базовый LIST-запрос delimiter и корневого имени mailbox hierarchy.
    Root,
    /// Delimiter discovery для непустой reference hierarchy.
    RootReference(String),
    All,
    OneLevel,
    Exact(String),
    Wildcard(String),
}

/// Критерии выбора initial extended LIST после сетевого парсинга.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ListSelection {
    pub subscribed: bool,
    pub special_use: bool,
    pub recursive_match: bool,
}

/// Указывает, какие дополнительные свойства должны присутствовать в LIST.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ListReturnOptions {
    /// Возвращать точный durable статус подписки каждого совпавшего имени.
    pub subscribed: bool,
    /// Возвращать известные enum Special-Use атрибуты mailbox.
    pub special_use: bool,
    /// Вычислять наличие дочерних mailbox по полному каталогу.
    pub children: bool,
    /// Упорядоченные STATUS data items для каждого совпавшего mailbox.
    pub status: Vec<super::status::StatusAttribute>,
}

impl ListReturnOptions {
    /// CHILDREN и SUBSCRIBED требуют сверки с полным snapshot каталога.
    pub fn needs_catalog_projection(&self) -> bool {
        !matches!(
            (self.subscribed, self.children, self.status.is_empty()),
            (false, false, true)
        )
    }

    /// Указывает, требуется ли построить mailbox status summaries в Store owner.
    pub fn needs_status_projection(&self) -> bool {
        !self.status.is_empty()
    }

    /// SIZE требует чтения только длин файлов, остальные счётчики берутся из metadata.
    pub fn needs_size_projection(&self) -> bool {
        self.status.contains(&super::status::StatusAttribute::Size)
    }
}

/// Операция нужна completion-слою для точного tagged-ответа.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mutation {
    Delete,
    Rename,
}

pub fn parse_create(mut tokens: impl Iterator<Item = Token>) -> Command {
    parse_create_with_mode(&mut tokens, super::tokenizer::StringMode::Legacy)
}

/// Разбирает CREATE, декодируя mailbox согласно уже выбранному режиму.
pub(super) fn parse_create_with_mode(
    tokens: &mut impl Iterator<Item = Token>,
    mode: super::tokenizer::StringMode,
) -> Command {
    let Some(name) = next_text(tokens) else {
        return Command::Unsupported;
    };
    let special_use = match tokens.next() {
        None => Some(None),
        Some(Token::ListStart) => parse_use(tokens).map(Some),
        _ => None,
    };
    match (mailbox_with_mode(name, mode), special_use) {
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
    parse_delete_with_mode(&mut tokens, super::tokenizer::StringMode::Legacy)
}

/// Разбирает DELETE в согласованном string mode.
pub(super) fn parse_delete_with_mode(
    tokens: &mut impl Iterator<Item = Token>,
    mode: super::tokenizer::StringMode,
) -> Command {
    match (next_text(tokens), tokens.next()) {
        (Some(name), None) => mailbox_with_mode(name, mode)
            .map(|mailbox| Command::Delete { mailbox })
            .unwrap_or(Command::Unsupported),
        _ => Command::Unsupported,
    }
}

pub fn parse_rename(mut tokens: impl Iterator<Item = Token>) -> Command {
    parse_rename_with_mode(&mut tokens, super::tokenizer::StringMode::Legacy)
}

/// Разбирает оба имени RENAME в одном согласованном string mode.
pub(super) fn parse_rename_with_mode(
    tokens: &mut impl Iterator<Item = Token>,
    mode: super::tokenizer::StringMode,
) -> Command {
    match (next_text(tokens), next_text(tokens), tokens.next()) {
        (Some(source), Some(destination), None) => match (
            mailbox_with_mode(source, mode),
            mailbox_with_mode(destination, mode),
        ) {
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
    parse_subscribe_with_mode(
        &mut tokens,
        subscribed,
        super::tokenizer::StringMode::Legacy,
    )
}

/// Разбирает SUBSCRIBE/UNSUBSCRIBE с mailbox, декодированным выбранным режимом.
pub(super) fn parse_subscribe_with_mode(
    tokens: &mut impl Iterator<Item = Token>,
    subscribed: bool,
    mode: super::tokenizer::StringMode,
) -> Command {
    let parsed = match (next_text(tokens), tokens.next()) {
        (Some(name), None) => mailbox_with_mode(name, mode),
        _ => None,
    };
    match (parsed, subscribed) {
        (Some(mailbox), true) => Command::Subscribe { mailbox },
        (Some(mailbox), false) => Command::Unsubscribe { mailbox },
        (None, _) => Command::Unsupported,
    }
}

/// Декодирует имя в явно выбранном wire-режиме, не меняя legacy callers.
pub(super) fn mailbox_with_mode(
    value: String,
    mode: super::tokenizer::StringMode,
) -> Option<MailboxName> {
    let decoded = match mode {
        super::tokenizer::StringMode::Legacy => super::modified_utf7::decode(&value).ok()?,
        super::tokenizer::StringMode::Utf8Accept => {
            super::mailbox_utf8::validate(&value).ok()?;
            crate::unicode::nfc::normalize(&value)
        }
    };
    match decoded.to_ascii_uppercase().as_str() {
        "INBOX" => Some(MailboxName::Inbox),
        _ => Some(MailboxName::Named(decoded)),
    }
}

fn next_text(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    match tokens.next()? {
        Token::Text(value) | Token::Quoted(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    }
}
