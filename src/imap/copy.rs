//! Типизированная grammar COPY и UID COPY.

use super::sequence_set::MessageSet;
use super::tokenizer::Token;
use super::{Addressing, Command, Transfer, folder};

/// Разбирает COPY/MOVE destination в явно выбранном string mode.
pub(super) fn parse_with_mode(
    tokens: &mut impl Iterator<Item = Token>,
    addressing: Addressing,
    transfer: Transfer,
    mode: super::tokenizer::StringMode,
) -> Command {
    match (next_text(tokens), next_text(tokens), tokens.next()) {
        (Some(sequence), Some(destination), None) => match (
            MessageSet::parse(&sequence).ok(),
            folder::mailbox_with_mode(destination, mode),
        ) {
            (Some(sequence_set), Some(destination)) => Command::Copy {
                transfer,
                addressing,
                sequence_set,
                destination,
            },
            _ => Command::Unsupported,
        },
        _ => Command::Unsupported,
    }
}

fn next_text(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    match tokens.next()? {
        Token::Text(value) | Token::Quoted(value) => Some(value),
        Token::Literal(value) => String::from_utf8(value).ok(),
        Token::ListStart | Token::ListEnd => None,
    }
}
