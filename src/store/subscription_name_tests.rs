//! Проверяет canonical alias handling для старых durable subscription names.

use super::SubscriptionCatalog;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn nfc_unsubscribe_removes_old_decomposed_subscription_record() {
    let root = std::env::temp_dir().join(format!(
        "litterkeep-subscription-nfc-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let subscription_root = root.join("subscriptions");
    std::fs::create_dir_all(&subscription_root).expect("subscription root");
    super::append(&subscription_root, "bob", "cafe\u{0301}", true)
        .expect("legacy subscription journal");
    let mut catalog = SubscriptionCatalog::load(&root).expect("load old subscription");

    assert_eq!(
        catalog.list("bob").expect("canonical projection"),
        vec!["café"]
    );
    catalog
        .set("bob", "café", false)
        .expect("unsubscribe by NFC alias");
    assert!(catalog.list("bob").expect("empty subscriptions").is_empty());

    let _ = std::fs::remove_dir_all(root);
}
