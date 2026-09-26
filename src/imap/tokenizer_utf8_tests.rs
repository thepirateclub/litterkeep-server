//! Unit-проверки грамматической границы RFC 9755 quoted-string mode.

use super::{StringMode, Token, tokenize_with_mode};

/// RFC 9755 high-bit расширяет quoted-string, но не atom грамматику.
#[test]
fn utf8_accept_mode_only_extends_quoted_strings() {
    assert_eq!(
        tokenize_with_mode("a CREATE \"Папка café\"\r\n", StringMode::Utf8Accept),
        Ok(vec![
            Token::Text("a".into()),
            Token::Text("CREATE".into()),
            Token::Quoted("Папка café".into()),
        ])
    );
    assert_eq!(
        tokenize_with_mode("a LOGIN \"bøb\" \"pass\"\r\n", StringMode::Legacy),
        Err("non-ASCII character in IMAP4rev1 quoted string")
    );
    assert_eq!(
        tokenize_with_mode("a CREATE Папка\r\n", StringMode::Utf8Accept),
        Err("non-ASCII character in IMAP4rev1 atom")
    );
}

/// UTF-8 extension не разрешает Unicode controls в quoted-string.
#[test]
fn utf8_accept_mode_rejects_unicode_control_scalars() {
    assert!(
        tokenize_with_mode(
            "a CREATE \"folder\u{0085}name\"\r\n",
            StringMode::Utf8Accept
        )
        .is_err()
    );
}
