//! Grammar и типизированные элементы команды STATUS.

use super::Command;
use super::folder::mailbox;
use super::tokenizer::Token;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusAttribute {
    Messages,
    UidNext,
    UidValidity,
    Unseen,
}

pub(super) fn parse(mut tokens: impl Iterator<Item = Token>) -> Command {
    let Some(name) = next_text(&mut tokens) else {
        return Command::Unsupported;
    };
    let Some(mailbox) = mailbox(name) else {
        return Command::Unsupported;
    };
    if !matches!(tokens.next(), Some(Token::ListStart)) {
        return Command::Unsupported;
    }
    let mut attributes = Vec::new();
    loop {
        match tokens.next() {
            Some(Token::Text(value)) => match parse_attribute(&value) {
                Some(attribute) => attributes.push(attribute),
                None => return Command::Unsupported,
            },
            Some(Token::ListEnd) => break,
            Some(Token::ListStart) | None => return Command::Unsupported,
        }
    }
    match (attributes.is_empty(), tokens.next()) {
        (false, None) => Command::Status {
            mailbox,
            attributes,
        },
        _ => Command::Unsupported,
    }
}

fn parse_attribute(value: &str) -> Option<StatusAttribute> {
    match value.to_ascii_uppercase().as_str() {
        "MESSAGES" => Some(StatusAttribute::Messages),
        "UIDNEXT" => Some(StatusAttribute::UidNext),
        "UIDVALIDITY" => Some(StatusAttribute::UidValidity),
        "UNSEEN" => Some(StatusAttribute::Unseen),
        _ => None,
    }
}

fn next_text(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    match tokens.next()? {
        Token::Text(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    }
}
