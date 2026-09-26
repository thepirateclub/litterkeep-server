//! Типизированный разбор согласования возможностей командой RFC ENABLE.

use super::Command;
use super::tokenizer::Token;

/// Атом capability преобразуется в Enum до передачи команды reactor-у.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Capability {
    Imap4rev2,
    Utf8Accept,
    Unsupported,
}

impl Capability {
    fn parse(value: &str) -> Self {
        match value.to_ascii_uppercase().as_str() {
            "IMAP4REV2" => Self::Imap4rev2,
            "UTF8=ACCEPT" => Self::Utf8Accept,
            _ => Self::Unsupported,
        }
    }
}

/// Разбирает непустой список atom; групповые формы отвергаются грамматикой.
pub(super) fn parse(tokens: impl Iterator<Item = Token>) -> Command {
    let mut capabilities = Vec::new();
    for token in tokens {
        match token {
            Token::Text(value) => capabilities.push(Capability::parse(&value)),
            Token::Quoted(_) | Token::Literal(_) | Token::ListStart | Token::ListEnd => {
                return Command::Unsupported;
            }
        }
    }
    match capabilities.is_empty() {
        true => Command::Unsupported,
        false => Command::Enable { capabilities },
    }
}
