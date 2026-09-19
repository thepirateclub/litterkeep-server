//! Типизированная grammar COPY и UID COPY.

use super::sequence_set::SequenceSet;
use super::tokenizer::Token;
use super::{Addressing, Command, folder};

/// Разбирает sequence-set и destination mailbox без строковой команды в reactor.
pub(super) fn parse(mut tokens: impl Iterator<Item = Token>, addressing: Addressing) -> Command {
    match (
        next_text(&mut tokens),
        next_text(&mut tokens),
        tokens.next(),
    ) {
        (Some(sequence), Some(destination), None) => match (
            SequenceSet::parse(&sequence).ok(),
            folder::mailbox(destination),
        ) {
            (Some(sequence_set), Some(destination)) => Command::Copy {
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
        Token::Text(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    }
}
