//! Проверки typed grammar команды STORE и saved-result selection.

use super::{Addressing, Command, parse};
use crate::imap::sequence_set::{MessageSet, SequenceSet};

/// STORE flags остаются enum-ами, включая системный saved sequence-set.
#[test]
fn store_flags_and_sequence_set_are_typed() {
    let Command::Store {
        addressing,
        sequence_set,
        mutation,
        silent,
        flags,
        keywords,
    } = parse("a8 UID STORE 4:9 +FLAGS.SILENT (\\Seen \\Flagged)\r\n").command
    else {
        panic!("STORE was not parsed");
    };
    assert_eq!(addressing, Addressing::Uid);
    assert_eq!(
        sequence_set,
        MessageSet::Explicit(SequenceSet::parse("4:9").unwrap())
    );
    assert_eq!(mutation, crate::mail_flags::FlagMutation::Add);
    assert!(silent);
    assert!(flags.contains(crate::mail_flags::SystemFlag::Seen));
    assert!(flags.contains(crate::mail_flags::SystemFlag::Flagged));
    assert!(keywords.is_empty());
}

/// STORE и UID STORE принимают saved-result только отдельным typed variant.
#[test]
fn saved_result_is_typed_for_uid_and_sequence_modes() {
    assert!(matches!(
        parse("a9 STORE $ +FLAGS (\\Seen)\r\n").command,
        Command::Store {
            addressing: Addressing::Sequence,
            sequence_set: MessageSet::Saved,
            ..
        }
    ));
    assert!(matches!(
        parse("a10 UID STORE $ +FLAGS (\\Seen)\r\n").command,
        Command::Store {
            addressing: Addressing::Uid,
            sequence_set: MessageSet::Saved,
            ..
        }
    ));
}
