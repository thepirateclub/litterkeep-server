//! Проверяет нормативные границы quoted-string и whitespace Sieve.

use super::{Token, tokenize};

/// RFC 5228 сохраняет CRLF в строке и снимает обратную косую черту у `\x`.
#[test]
fn quoted_string_preserves_crlf_and_accepts_undefined_escape() {
    assert_eq!(
        tokenize("\"first\r\nsecond\"").expect("строка с CRLF"),
        vec![Token::String("first\r\nsecond".into())]
    );
    assert_eq!(
        tokenize("\"foo\\abar\"").expect("неопределённый escape"),
        vec![Token::String("fooabar".into())]
    );
}

/// NUL и непарные CR/LF запрещены как в строке, так и между токенами.
#[test]
fn quoted_string_rejects_nul_and_bare_line_endings() {
    assert!(tokenize("\"a\0b\"").is_err());
    assert!(tokenize("\"a\rb\"").is_err());
    assert!(tokenize("\"a\nb\"").is_err());
    assert!(tokenize("keep;\rdiscard;").is_err());
    assert!(tokenize("keep;\ndiscard;").is_err());
}
