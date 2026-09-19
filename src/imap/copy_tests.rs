//! Проверки parser-а COPY в отдельной предметной области.

use super::*;

/// COPY сохраняет отдельно sequence number/UID addressing и destination mailbox.
#[test]
fn copy_is_typed_for_sequence_and_uid_modes() {
    let Command::Copy {
        addressing,
        sequence_set,
        destination,
    } = parse("a COPY 2:1 Sent\r\n").command
    else {
        panic!("COPY was not parsed");
    };
    assert_eq!(addressing, Addressing::Sequence);
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
