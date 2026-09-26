use super::*;

/// APPEND принимает INTERNALDATE только в его RFC quoted-string форме.
#[test]
fn append_rejects_unquoted_internal_date() {
    assert_eq!(
        parse("A1 APPEND INBOX 19-Sep-2026 12:34:56 +0300 {0}\r\n").command,
        Command::Unsupported
    );
}

/// Quoted INTERNALDATE и последующий literal остаются типизированными.
#[test]
fn append_accepts_quoted_internal_date() {
    assert!(matches!(
        parse("A2 APPEND INBOX \"19-Sep-2026 12:34:56 +0300\" {0}\r\n").command,
        Command::Append {
            literal_size: 0,
            literal_mode: crate::imap::literal::Mode::Synchronizing,
            internal_date: Some(_),
            ..
        }
    ));
}

/// APPEND разбирает LITERAL- marker в typed non-synchronizing mode.
#[test]
fn append_accepts_non_synchronizing_literal_marker() {
    assert!(matches!(
        parse("A4 APPEND INBOX {4+}\r\n").command,
        Command::Append {
            literal_size: 4,
            literal_mode: crate::imap::literal::Mode::NonSynchronizing,
            ..
        }
    ));
}

/// Лишний аргумент после literal marker не скрывается разбором даты.
#[test]
fn append_rejects_argument_after_literal_marker() {
    assert_eq!(
        parse("A3 APPEND INBOX {0} extra\r\n").command,
        Command::Unsupported
    );
}
