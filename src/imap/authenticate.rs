//! Типизированная grammar IMAP AUTHENTICATE и SASL initial response.

use super::{Command, tokenizer::Token};

/// SASL mechanism распознаётся до reactor boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticationMechanism {
    Plain,
}

/// SASL-IR отделяет отсутствие ответа от переданного пустого значения `=`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InitialResponse {
    Absent,
    Present(String),
}

/// Принимает PLAIN без ответа либо с одним SASL-IR atom.
pub(super) fn parse(tokens: &mut impl Iterator<Item = Token>) -> Command {
    let mechanism = match tokens.next() {
        Some(Token::Text(value) | Token::Quoted(value)) if value.eq_ignore_ascii_case("PLAIN") => {
            AuthenticationMechanism::Plain
        }
        _ => return Command::Unsupported,
    };
    match (tokens.next(), tokens.next()) {
        (None, None) => Command::Authenticate {
            mechanism,
            initial_response: InitialResponse::Absent,
        },
        (Some(Token::Text(initial_response) | Token::Quoted(initial_response)), None)
            if initial_response == "=" =>
        {
            Command::Authenticate {
                mechanism,
                initial_response: InitialResponse::Present(String::new()),
            }
        }
        (Some(Token::Text(initial_response) | Token::Quoted(initial_response)), None) => {
            Command::Authenticate {
                mechanism,
                initial_response: InitialResponse::Present(initial_response),
            }
        }
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
                initial_response: InitialResponse::Present("AGJvYgB0ZXN0".into()),
            }
        );
        assert_eq!(
            parse("b AUTHENTICATE plain\r\n").command,
            Command::Authenticate {
                mechanism: AuthenticationMechanism::Plain,
                initial_response: InitialResponse::Absent,
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
        assert_eq!(
            parse("e AUTHENTICATE \"PLAIN\" \"AGJvYgB0ZXN0\"\r\n").command,
            Command::Authenticate {
                mechanism: AuthenticationMechanism::Plain,
                initial_response: InitialResponse::Present("AGJvYgB0ZXN0".into()),
            }
        );
        assert_eq!(
            parse("f AUTHENTICATE PLAIN =\r\n").command,
            Command::Authenticate {
                mechanism: AuthenticationMechanism::Plain,
                initial_response: InitialResponse::Present(String::new()),
            }
        );
    }

    /// Пустой response marker не смешивается с запросом continuation.
    #[test]
    fn empty_initial_response_differs_from_absent_response() {
        let empty = parse("a AUTHENTICATE PLAIN =\r\n").command;
        let absent = parse("b AUTHENTICATE PLAIN\r\n").command;
        assert_eq!(
            empty,
            Command::Authenticate {
                mechanism: AuthenticationMechanism::Plain,
                initial_response: InitialResponse::Present(String::new()),
            }
        );
        assert_eq!(
            absent,
            Command::Authenticate {
                mechanism: AuthenticationMechanism::Plain,
                initial_response: InitialResponse::Absent,
            }
        );
    }
}
