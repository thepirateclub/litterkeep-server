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
        Command::AppendStart {
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
        Command::AppendStart {
            literal_size: 4,
            literal_mode: crate::imap::literal::Mode::NonSynchronizing,
            ..
        }
    ));
}

/// Полный parser сохраняет порядок сообщений и независимые metadata.
#[test]
fn append_parses_multiple_literal_messages_in_order() {
    let parsed = crate::imap::parse_literals_with_mode(
        "A5 APPEND INBOX (\\Seen) \0 (\\Draft) \"19-Sep-2026 12:34:56 +0300\" \0\r\n",
        vec![b"first".to_vec(), b"second".to_vec()],
        crate::imap::tokenizer::StringMode::Legacy,
    );
    let Command::Append { messages, .. } = parsed.command else {
        panic!("MULTIAPPEND literals were not parsed");
    };
    let first = messages.first().expect("first message");
    let second = messages.get(1).expect("second message");
    assert_eq!(first.bytes, b"first");
    assert!(first.flags.contains(crate::mail_flags::SystemFlag::Seen));
    assert_eq!(second.bytes, b"second");
    assert!(second.flags.contains(crate::mail_flags::SystemFlag::Draft));
    assert!(second.internal_date.is_some());
}

/// Лишний аргумент после literal marker не скрывается разбором даты.
#[test]
fn append_rejects_argument_after_literal_marker() {
    assert_eq!(
        parse("A3 APPEND INBOX {0} extra\r\n").command,
        Command::Unsupported
    );
}
