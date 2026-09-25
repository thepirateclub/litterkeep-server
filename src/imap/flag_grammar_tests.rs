use super::{Addressing, Command, MailboxName, parse};
use crate::mail_flags::SystemFlag;

/// STORE отклоняет непарные и вложенные скобки вместо их удаления из потока.
#[test]
fn store_rejects_malformed_flag_list_boundaries() {
    for line in [
        "A1 STORE 1 FLAGS (\\Seen\r\n",
        "A2 STORE 1 FLAGS \\Seen)\r\n",
        "A3 STORE 1 FLAGS ((\\Seen))\r\n",
        "A4 STORE 1 FLAGS \"\\Seen\"\r\n",
    ] {
        assert_eq!(parse(line).command, Command::Unsupported, "{line:?}");
    }
}

/// STORE сохраняет обе допустимые формы списка флагов.
#[test]
fn store_accepts_valid_flag_list_forms() {
    for line in ["A5 STORE 1 FLAGS (\\Seen)", "A6 STORE 1 FLAGS \\Seen"] {
        assert!(matches!(
            parse(line).command,
            Command::Store {
                addressing: Addressing::Sequence,
                flags,
                ..
            } if flags.contains(SystemFlag::Seen)
        ));
    }
}

/// APPEND flag-list принимает system atom, но не quoted flag и неверные delimiters.
#[test]
fn append_rejects_quoted_and_unclosed_flags() {
    assert_eq!(
        parse("A7 APPEND INBOX (\"\\Seen\") {0}\r\n").command,
        Command::Unsupported
    );
    assert_eq!(
        parse("A8 APPEND INBOX (\\Seen {0}\r\n").command,
        Command::Unsupported
    );
    assert!(matches!(
        parse("A9 APPEND INBOX (\\Seen) {0}\r\n").command,
        Command::Append {
            mailbox: MailboxName::Inbox,
            flags,
            ..
        } if flags.contains(SystemFlag::Seen)
    ));
}
