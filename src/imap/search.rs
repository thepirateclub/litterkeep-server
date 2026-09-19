//! Строгий начальный профиль IMAP SEARCH без ложных пустых результатов.

use super::tokenizer::Token;
use super::{Addressing, Command};

/// Поддерживаемые предикаты над текущим набором системных flags.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchKey {
    All,
    Seen,
    Unseen,
    Deleted,
    Undeleted,
}

/// Разбирает ровно один базовый search key, иначе возвращает BAD через parser.
pub fn parse(mut tokens: impl Iterator<Item = Token>, addressing: Addressing) -> Command {
    let Some(Token::Text(value)) = tokens.next() else {
        return Command::Unsupported;
    };
    if tokens.next().is_some() {
        return Command::Unsupported;
    }
    let key = match value.to_ascii_uppercase().as_str() {
        "ALL" => SearchKey::All,
        "SEEN" => SearchKey::Seen,
        "UNSEEN" => SearchKey::Unseen,
        "DELETED" => SearchKey::Deleted,
        "UNDELETED" => SearchKey::Undeleted,
        _ => return Command::Unsupported,
    };
    Command::Search { addressing, key }
}
