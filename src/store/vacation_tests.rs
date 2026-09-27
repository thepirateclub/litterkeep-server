//! Проверки атомарности durable Sieve vacation suppression.

use super::{
    AppendStage, Store, StoreFailpoint,
    vacation_access::{VacationResponseRequest, VacationResponseResult},
};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn vacation_claim_and_null_sender_job_survive_reopen() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    assert!(matches!(
        store.enqueue_vacation_response(request(b"reply", 100, "reply-v1")),
        Ok(VacationResponseResult::Queued { .. })
    ));
    assert!(matches!(
        store.enqueue_vacation_response(request(b"ignored", 101, "reply-v1")),
        Ok(VacationResponseResult::Suppressed)
    ));
    drop(store);

    let mut reopened = Store::open(&root).expect("reopen");
    assert!(matches!(
        reopened.enqueue_vacation_response(request(b"ignored", 200, "reply-v1")),
        Ok(VacationResponseResult::Suppressed)
    ));
    let delivery = reopened
        .pending_outbound()
        .expect("jobs")
        .into_iter()
        .next()
        .expect("job");
    assert_eq!(delivery.sender, "");
    assert_eq!(
        reopened.outbound_blob(&delivery).expect("response"),
        b"reply"
    );
    drop(reopened);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn uncommitted_vacation_claim_does_not_suppress_after_recovery() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    store.set_failpoint(StoreFailpoint::Stage(AppendStage::VacationWritten));
    assert!(
        store
            .enqueue_vacation_response(request(b"reply", 100, "reply-v1"))
            .is_err()
    );
    drop(store);
    let mut reopened = Store::open(&root).expect("recover");
    assert!(reopened.pending_outbound().expect("jobs").is_empty());
    assert!(matches!(
        reopened.enqueue_vacation_response(request(b"reply", 101, "reply-v1")),
        Ok(VacationResponseResult::Queued { .. })
    ));
    drop(reopened);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn vacation_expiry_and_response_identity_allow_new_claims() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    assert!(matches!(
        store.enqueue_vacation_response(request(b"first", 10, "reply-v1")),
        Ok(VacationResponseResult::Queued { .. })
    ));
    assert!(matches!(
        store.enqueue_vacation_response(request(b"expired", 604_811, "reply-v1")),
        Ok(VacationResponseResult::Queued { .. })
    ));
    assert!(matches!(
        store.enqueue_vacation_response(request(b"changed", 604_812, "reply-v2")),
        Ok(VacationResponseResult::Queued { .. })
    ));
    drop(store);
    fs::remove_dir_all(root).expect("cleanup");
}

fn request<'a>(raw: &'a [u8], now_epoch: u64, identity: &'a str) -> VacationResponseRequest<'a> {
    VacationResponseRequest {
        account: "bob",
        sender: "alice@peer.test",
        response_identity: identity,
        recipient: "alice@peer.test",
        raw,
        now_epoch,
        interval_seconds: 604_800,
    }
}

fn directory() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("litterkeep-vacation-{nonce}"))
}
