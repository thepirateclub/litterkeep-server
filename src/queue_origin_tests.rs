//! Проверки совместимости и границ агрегированного RCPT-контекста очереди.

use super::Queue;
use std::{collections::HashSet, fs, io};

/// Старый journal с одним ordinal остаётся читаемым после расширения формата.
#[test]
fn legacy_single_recipient_ordinal_recovers_as_one_origin() {
    let root = std::env::temp_dir().join("litterkeep-queue-legacy-origin-test");
    let queue = Queue::open(&root).expect("open queue");
    fs::write(&queue.path, b"pending 52 0 0 alice@test remote@test 4 -\n")
        .expect("write legacy record");
    let committed = HashSet::from([52]);

    let recovered = Queue::open(&root)
        .expect("reopen queue")
        .pending_owned(&committed)
        .expect("recover legacy record");

    assert_eq!(recovered.len(), 1);
    assert_eq!(
        recovered
            .first()
            .map(|job| job.original_recipient_indices.clone()),
        Some(vec![4])
    );
    let _ = fs::remove_dir_all(root);
}

/// Непоследовательные и дублированные ordinals не записываются в journal.
#[test]
fn enqueue_rejects_duplicate_or_unordered_origin_indices() {
    let root = std::env::temp_dir().join("litterkeep-queue-invalid-origin-test");
    let queue = Queue::open(&root).expect("open queue");
    for origins in [&[2, 1][..], &[1, 1][..]] {
        let error = queue
            .enqueue_owned_many(53, "alice@test", "remote@test", origins)
            .expect_err("invalid origin ordering");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
    assert!(!queue.path.exists());
    let _ = fs::remove_dir_all(root);
}

/// Ограничивает одну очередь количеством исходных RCPT SMTP transaction.
#[test]
fn enqueue_rejects_more_origins_than_smtp_transaction_limit() {
    let root = std::env::temp_dir().join("litterkeep-queue-origin-limit-test");
    let queue = Queue::open(&root).expect("open queue");
    let origins = (0..=100).collect::<Vec<_>>();
    let error = queue
        .enqueue_owned_many(54, "alice@test", "remote@test", &origins)
        .expect_err("origin limit");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(!queue.path.exists());
    let _ = fs::remove_dir_all(root);
}

/// Список origin-ов сохраняется при записи transport retry state.
#[test]
fn retry_transition_replays_all_origin_indices() {
    let root = std::env::temp_dir().join("litterkeep-queue-origin-transition-test");
    let queue = Queue::open(&root).expect("open queue");
    queue
        .enqueue_owned_many(55, "alice@test", "remote@test", &[0, 2, 5])
        .expect("enqueue grouped route");
    let committed = HashSet::from([55]);
    let mut delivery = queue
        .pending_owned(&committed)
        .expect("pending")
        .into_iter()
        .next()
        .expect("route");
    delivery.attempts = 1;
    delivery.next_attempt_epoch = 60;
    delivery.state = super::State::Deferred;
    queue
        .transition_owned(&delivery, super::State::Deferred)
        .expect("write transition");

    let recovered = Queue::open(&root)
        .expect("reopen queue")
        .pending_owned(&committed)
        .expect("replay transition");

    assert_eq!(
        recovered
            .first()
            .map(|item| item.original_recipient_indices.clone()),
        Some(vec![0, 2, 5])
    );
    assert_eq!(
        recovered.first().map(|item| item.state),
        Some(super::State::Deferred)
    );
    let _ = fs::remove_dir_all(root);
}
