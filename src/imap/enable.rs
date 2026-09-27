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
            Token::Text(value) => {
                let Some(atom) = super::token_values::atom_value(Token::Text(value)) else {
                    return Command::Unsupported;
                };
                capabilities.push(Capability::parse(&atom));
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imap;

    /// Неизвестный допустимый atom игнорируется, ошибочный atom отклоняется.
    #[test]
    fn enable_separates_unknown_capabilities_from_invalid_atoms() {
        assert_eq!(
            imap::parse("a ENABLE X-UNKNOWN\r\n").command,
            Command::Enable {
                capabilities: vec![Capability::Unsupported],
            }
        );
        for value in ["%", "X*", "X]", "{", "\\"] {
            let command = format!("a ENABLE {value}\r\n");
            assert_eq!(imap::parse(&command).command, Command::Unsupported);
        }
    }
}
