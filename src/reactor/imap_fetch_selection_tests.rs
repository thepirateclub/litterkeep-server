//! Проверки PARTIAL rank считают только сообщения исходного UID set.

use super::*;

#[test]
fn newest_range_is_relative_to_matching_uid_fetch_results() {
    let messages = (1..=6).map(message).collect::<Vec<_>>();
    let set = imap::sequence_set::MessageSet::Explicit(
        imap::sequence_set::SequenceSet::parse("2,4,6").unwrap(),
    );
    let range = positions(
        imap::Addressing::Uid,
        &set,
        Some(&imap::partial_range::PartialRange::parse("-1:-2").unwrap()),
        &messages,
        &BTreeSet::new(),
    );
    assert_eq!(range, 1..3);
}

fn message(uid: u64) -> crate::store::Message {
    crate::store::Message {
        uid,
        path: std::path::PathBuf::new(),
        bytes: Vec::new(),
        flags: crate::mail_flags::MessageFlags::default(),
        keywords: BTreeSet::new(),
        internal_date: crate::mail_date::MailDate::from_unix_seconds(0),
    }
}
