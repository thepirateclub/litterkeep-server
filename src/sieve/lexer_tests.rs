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

#[test]
fn comments_require_crlf_and_reject_nul() {
    assert!(tokenize("keep; # comment\r\nkeep;").is_ok());
    assert!(tokenize("keep; # comment\nkeep;").is_err());
    assert!(tokenize("keep; # comment\rkeep;").is_err());
    assert!(tokenize("keep; # comment").is_err());
    assert!(tokenize("/* comment\0 */ keep;").is_err());
    assert!(tokenize("/* comment\n */ keep;").is_err());
}

/// Многострочная строка сохраняет строки CRLF и снимает только dot-stuffing.
#[test]
fn multiline_string_preserves_payload_and_unstuffs_leading_dots() {
    assert_eq!(
        tokenize("text:\r\nfirst\r\n..dot\r\n.dot\r\n.\r\n").expect("многострочная строка"),
        vec![Token::String("first\r\n.dot\r\n.dot\r\n".into())]
    );
    assert_eq!(
        tokenize("text:\r\n.\r\n").expect("пустая строка"),
        vec![Token::String(String::new())]
    );
}

/// Разделитель допускает SP/HTAB и hash-comment, но не bracket-comment.
#[test]
fn multiline_string_accepts_only_defined_header_whitespace() {
    assert_eq!(
        tokenize("text: \t# comment\r\nbody\r\n.\r\n").expect("header comment"),
        vec![Token::String("body\r\n".into())]
    );
    assert!(tokenize("text: /* comment */\r\nbody\r\n.\r\n").is_err());
    assert!(tokenize("text: body\r\n.\r\n").is_err());
    assert!(tokenize("text:\r\nbody\r\n").is_err());
}

/// `text:` проходит тот же parser и validator, что и сохранённые scripts.
#[test]
fn multiline_string_is_accepted_as_a_test_argument() {
    assert!(
        crate::sieve::parse("if header :contains \"Subject\" text:\r\nInvoice\r\n.\r\n { keep; }")
            .is_ok()
    );
}
