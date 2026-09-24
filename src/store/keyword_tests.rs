//! Проверки полного жизненного цикла persistent IMAP keyword flags.

use super::*;
use crate::mail_flags::KeywordFlag;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

/// Пользовательские keywords проходят APPEND, STORE, COPY и восстановление.
#[test]
fn keyword_flags_are_atomic_durable_and_copied_with_the_message() {
    let path = std::env::temp_dir().join(format!(
        "litterkeep-keywords-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let mut store = Store::open(&path).expect("store");
    let initial = KeywordFlag::parse("$Forwarded").expect("keyword");
    let added = KeywordFlag::parse("Invoices2026").expect("keyword");
    let metadata = AppendMetadata {
        flags: MessageFlags::default(),
        keywords: [initial.clone()].into(),
        internal_date: crate::mail_date::MailDate::from_unix_seconds(1_790_000_000),
    };
    store
        .append_with_metadata("source", b"message", metadata)
        .expect("append");
    store
        .update_stored_flags(
            "source",
            &[1],
            FlagMutation::Add,
            MessageFlags::default(),
            &[added.clone()].into(),
        )
        .expect("add keyword");
    store.copy("source", "destination", &[1]).expect("copy");
    drop(store);

    let reopened = Store::open(&path).expect("reopen");
    let source = reopened.snapshot("source").expect("source");
    let destination = reopened.snapshot("destination").expect("destination");
    let expected = [initial, added].into();
    assert_eq!(
        source.messages.first().map(|message| &message.keywords),
        Some(&expected)
    );
    assert_eq!(
        destination
            .messages
            .first()
            .map(|message| &message.keywords),
        Some(&expected)
    );
    drop(reopened);
    fs::remove_dir_all(path).expect("cleanup");
}
