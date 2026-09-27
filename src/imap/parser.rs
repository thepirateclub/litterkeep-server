//! Маршрутизация IMAP токенов в типизированные команды протокола.

use super::command_verb::{Verb, parse_verb};
use super::token_values;
use super::tokenizer::{self, StringMode, Token};
use super::{Addressing, Command, TaggedCommand};

/// Разбирает одну командную строку через lexer и типизированный dispatch.
pub(super) fn parse(line: &str) -> TaggedCommand {
    parse_with_mode(line, StringMode::Legacy)
}

/// Разбирает одну команду после выбора клиентского string mode.
pub(super) fn parse_with_mode(line: &str, mode: StringMode) -> TaggedCommand {
    parse_tokens(line, tokenizer::tokenize_with_mode(line, mode), mode)
}

/// Разбирает проверенные literals, сохраняя согласованный режим прочих строк.
#[cfg(any(target_os = "linux", test))]
pub(super) fn parse_literals_with_mode(
    line: &str,
    literals: Vec<Vec<u8>>,
    mode: StringMode,
) -> TaggedCommand {
    parse_tokens(
        line,
        tokenizer::tokenize_literals_with_mode(line, literals, mode),
        mode,
    )
}

/// Преобразует lexer result в команду, сохраняя tag даже при синтаксическом отказе.
fn parse_tokens(
    line: &str,
    result: Result<Vec<Token>, &'static str>,
    mode: StringMode,
) -> TaggedCommand {
    let mut tokens = match result {
        Ok(tokens) => tokens.into_iter(),
        Err(_) => {
            let tag = line
                .split_ascii_whitespace()
                .next()
                .filter(|candidate| token_values::valid_tag(candidate))
                .unwrap_or("*");
            return unsupported(tag);
        }
    };
    let tag = match tokens.next() {
        Some(Token::Text(tag)) if token_values::valid_tag(&tag) => tag,
        _ => return unsupported("*"),
    };
    let command = match tokens
        .next()
        .and_then(token_values::atom_value)
        .as_deref()
        .map(parse_verb)
    {
        Some(Verb::Capability) => parse_empty(tokens, Command::Capability),
        Some(Verb::Namespace) => parse_empty(tokens, Command::Namespace),
        Some(Verb::Enable) => super::enable::parse(tokens),
        Some(Verb::StartTls) => parse_empty(tokens, Command::StartTls),
        Some(Verb::Login) => parse_login(tokens),
        Some(Verb::Authenticate) => super::authenticate::parse(&mut tokens),
        Some(Verb::Append) => super::append::parse_with_mode(tokens, mode),
        Some(Verb::Logout) => parse_empty(tokens, Command::Logout),
        Some(Verb::Noop) => parse_empty(tokens, Command::Noop),
        Some(Verb::Idle) => parse_empty(tokens, Command::Idle),
        Some(Verb::Create) => super::folder::parse_create_with_mode(&mut tokens, mode),
        Some(Verb::Delete) => super::folder::parse_delete_with_mode(&mut tokens, mode),
        Some(Verb::Rename) => super::folder::parse_rename_with_mode(&mut tokens, mode),
        Some(Verb::Subscribe) => super::folder::parse_subscribe_with_mode(&mut tokens, true, mode),
        Some(Verb::Unsubscribe) => {
            super::folder::parse_subscribe_with_mode(&mut tokens, false, mode)
        }
        Some(Verb::Lsub) => super::list::parse_lsub_with_mode(&mut tokens, mode),
        Some(Verb::Status) => super::status::parse_with_mode(&mut tokens, mode),
        Some(Verb::Check) => parse_empty(tokens, Command::Check),
        Some(Verb::Close) => parse_empty(tokens, Command::Close),
        Some(Verb::Unselect) => parse_empty(tokens, Command::Unselect),
        Some(Verb::Copy) => super::copy::parse_with_mode(
            &mut tokens,
            Addressing::Sequence,
            super::Transfer::Copy,
            mode,
        ),
        Some(Verb::Move) => super::copy::parse_with_mode(
            &mut tokens,
            Addressing::Sequence,
            super::Transfer::Move,
            mode,
        ),
        Some(Verb::Expunge) => parse_empty(tokens, Command::Expunge),
        Some(Verb::List) => super::list::parse_with_mode(&mut tokens, mode),
        Some(Verb::Select) => parse_select(&mut tokens, false, mode),
        Some(Verb::Examine) => parse_select(&mut tokens, true, mode),
        Some(Verb::Fetch) => super::fetch::parse(&mut tokens, Addressing::Sequence),
        Some(Verb::Store) => super::flags::parse_store(&mut tokens, Addressing::Sequence),
        Some(Verb::Search) => super::search::parse(tokens, Addressing::Sequence),
        Some(Verb::Uid) => parse_uid(&mut tokens, mode),
        Some(Verb::Unsupported) | None => Command::Unsupported,
    };
    TaggedCommand { tag, command }
}

/// LOGIN требует два корректных astring, atom или quoted-string каждый.
fn parse_login(mut tokens: impl Iterator<Item = Token>) -> Command {
    match (
        next_astring(&mut tokens),
        tokens.next().and_then(token_values::astring_bytes),
        tokens.next(),
    ) {
        (Some(username), Some(password), None) => Command::Login { username, password },
        _ => Command::Unsupported,
    }
}

/// Разбирает подкоманду UID как обязательный atom.
fn parse_uid(tokens: &mut impl Iterator<Item = Token>, mode: StringMode) -> Command {
    match tokens
        .next()
        .and_then(token_values::atom_value)
        .as_deref()
        .map(parse_verb)
    {
        Some(Verb::Fetch) => super::fetch::parse(tokens, Addressing::Uid),
        Some(Verb::Store) => super::flags::parse_store(tokens, Addressing::Uid),
        Some(Verb::Search) => super::search::parse(tokens, Addressing::Uid),
        Some(Verb::Copy) => {
            super::copy::parse_with_mode(tokens, Addressing::Uid, super::Transfer::Copy, mode)
        }
        Some(Verb::Move) => {
            super::copy::parse_with_mode(tokens, Addressing::Uid, super::Transfer::Move, mode)
        }
        Some(Verb::Expunge) => parse_uid_expunge(tokens),
        _ => Command::Unsupported,
    }
}

/// UID EXPUNGE требует один sequence-set в atom-форме и без хвостовых токенов.
fn parse_uid_expunge(mut tokens: impl Iterator<Item = Token>) -> Command {
    match (
        tokens.next().and_then(token_values::atom_value),
        tokens.next(),
    ) {
        (Some(value), None) => super::sequence_set::MessageSet::parse(&value)
            .map(|sequence_set| Command::UidExpunge { sequence_set })
            .unwrap_or(Command::Unsupported),
        _ => Command::Unsupported,
    }
}

/// SELECT и EXAMINE разбирают mailbox как astring после выбора режима.
fn parse_select(
    tokens: &mut impl Iterator<Item = Token>,
    read_only: bool,
    mode: StringMode,
) -> Command {
    let Some(mailbox) =
        next_astring(tokens).and_then(|value| super::folder::mailbox_with_mode(value, mode))
    else {
        return Command::Unsupported;
    };
    match read_only {
        true => Command::Examine { mailbox },
        false => Command::Select { mailbox },
    }
}

/// Достаёт atom или quoted-string, не перескакивая через структурные токены.
fn next_astring(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    tokens.next().and_then(token_values::astring_value)
}

/// Безаргументная команда не должна оставлять непрочитанные токены.
fn parse_empty(mut tokens: impl Iterator<Item = Token>, command: Command) -> Command {
    match tokens.next() {
        None => command,
        Some(_) => Command::Unsupported,
    }
}

/// Некорректная команда сохраняет только безопасный tag либо untagged fallback.
fn unsupported(tag: &str) -> TaggedCommand {
    TaggedCommand {
        tag: tag.into(),
        command: Command::Unsupported,
    }
}
