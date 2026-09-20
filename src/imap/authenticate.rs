//! Типизированная grammar IMAP AUTHENTICATE и SASL initial response.

use super::{Command, tokenizer::Token};

/// SASL mechanism распознаётся до reactor boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticationMechanism {
    Plain,
}

/// Принимает PLAIN без ответа либо с одним SASL-IR atom.
pub(super) fn parse(tokens: &mut impl Iterator<Item = Token>) -> Command {
    let mechanism = match tokens.next() {
        Some(Token::Text(value)) if value.eq_ignore_ascii_case("PLAIN") => {
            AuthenticationMechanism::Plain
        }
        _ => return Command::Unsupported,
    };
    match (tokens.next(), tokens.next()) {
        (None, None) => Command::Authenticate {
            mechanism,
            initial_response: None,
        },
        (Some(Token::Text(initial_response)), None) => Command::Authenticate {
            mechanism,
            initial_response: Some(initial_response),
        },
        _ => Command::Unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::super::parse;
    use super::*;

    /// Mechanism и optional initial response не остаются строковой командой.
    #[test]
    fn plain_is_typed_with_optional_initial_response() {
        assert_eq!(
            parse("a AUTHENTICATE PLAIN AGJvYgB0ZXN0\r\n").command,
            Command::Authenticate {
                mechanism: AuthenticationMechanism::Plain,
                initial_response: Some("AGJvYgB0ZXN0".into()),
            }
        );
        assert_eq!(
            parse("b AUTHENTICATE plain\r\n").command,
            Command::Authenticate {
                mechanism: AuthenticationMechanism::Plain,
                initial_response: None,
            }
        );
        assert_eq!(
            parse("c AUTHENTICATE XOAUTH2 token\r\n").command,
            Command::Unsupported
        );
        assert_eq!(
            parse("d AUTHENTICATE (PLAIN)\r\n").command,
            Command::Unsupported
        );
    }
}
