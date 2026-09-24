//! Проверки SEARCH RETURN grammar и ESEARCH compact sequence-set.

use super::*;

#[test]
fn parses_extended_result_items_and_defaults_empty_list_to_all() {
    let parsed = crate::imap::parse("a SEARCH RETURN (COUNT MIN MAX) ALL\r\n");
    assert!(matches!(
        parsed.command,
        crate::imap::Command::Search {
            result: ResultRequest::Extended(items),
            ..
        } if items == vec![ReturnItem::Count, ReturnItem::Min, ReturnItem::Max]
    ));
    let empty = crate::imap::parse("b UID SEARCH RETURN () ALL\r\n");
    assert!(matches!(
        empty.command,
        crate::imap::Command::Search {
            addressing: crate::imap::Addressing::Uid,
            result: ResultRequest::Extended(items),
            ..
        } if items == vec![ReturnItem::All]
    ));
}

#[test]
fn parses_save_alongside_result_items() {
    let command = crate::imap::parse("a SEARCH RETURN (SAVE MIN) ALL\r\n").command;
    assert!(matches!(
        command,
        crate::imap::Command::Search {
            result: ResultRequest::Extended(items),
            ..
        } if items == vec![ReturnItem::Save, ReturnItem::Min]
    ));
}

#[test]
fn rejects_unknown_or_malformed_return_items() {
    for line in [
        "a SEARCH RETURN (PARTIAL 1:2) ALL\r\n",
        "b SEARCH RETURN ALL\r\n",
        "c SEARCH RETURN (\"COUNT\") ALL\r\n",
        "d SEARCH RETURN (COUNT COUNT) ALL\r\n",
    ] {
        assert_eq!(
            crate::imap::parse(line).command,
            crate::imap::Command::Unsupported
        );
    }
}

#[test]
fn compacts_sorted_values_without_creating_incomplete_ranges() {
    assert_eq!(sequence_set(&[1, 2, 3, 5, 7, 8]), "1:3,5,7:8");
    assert_eq!(sequence_set(&[]), "");
    assert_eq!(sequence_set(&[4]), "4");
}
