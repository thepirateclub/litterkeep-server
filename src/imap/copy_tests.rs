//! Проверки parser-а COPY в отдельной предметной области.

use super::*;

/// COPY сохраняет отдельно sequence number/UID addressing и destination mailbox.
#[test]
fn copy_is_typed_for_sequence_and_uid_modes() {
    let Command::Copy {
        transfer,
        addressing,
        sequence_set,
        destination,
    } = parse("a COPY 2:1 Sent\r\n").command
    else {
        panic!("COPY was not parsed");
    };
    assert_eq!(addressing, Addressing::Sequence);
    assert_eq!(transfer, Transfer::Copy);
    assert_eq!(sequence_set, SequenceSet::parse("2:1").expect("set"));
    assert_eq!(destination, MailboxName::Named("Sent".into()));
    assert!(matches!(
        parse("a UID COPY 4 Archive\r\n").command,
        Command::Copy {
            addressing: Addressing::Uid,
            ..
        }
    ));
}

#[test]
fn move_is_typed_for_sequence_and_uid_modes() {
    let TaggedCommand { command, .. } = parse("a UID MOVE 2:1 Archive\r\n");
    let Command::Copy {
        transfer,
        addressing,
        ..
    } = command
    else {
        panic!("MOVE command")
    };
    assert_eq!(transfer, Transfer::Move);
    assert_eq!(addressing, Addressing::Uid);
}

/// Extended LIST выделяет SPECIAL-USE selection до reactor-а.
#[test]
fn special_use_list_and_wildcard_are_typed() {
    assert!(matches!(
        parse("a LIST (SPECIAL-USE) \"\" \"*\"\r\n").command,
        Command::ListSpecial {
            pattern: crate::imap::folder::ListPattern::All
        }
    ));
    assert!(matches!(
        parse("a LIST \"\" \"Projects/%\"\r\n").command,
        Command::List {
            pattern: crate::imap::folder::ListPattern::Wildcard(_)
        }
    ));
}

#[test]
fn list_return_special_use_keeps_the_requested_selection() {
    assert!(matches!(
        parse("a LIST \"\" \"*\" RETURN (SPECIAL-USE)\r\n").command,
        Command::List {
            pattern: folder::ListPattern::All
        }
    ));
}
