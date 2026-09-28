//! Проверки durable queue journal с реальным reopen каталога.
use crate::queue::{Delivery, OwnedDelivery, Queue, State};
use std::{collections::HashSet, fs};

#[test]
fn replay_survives_reopen() {
    let d = std::env::temp_dir().join("litterkeep-queue-test");
    let q = Queue::open(&d).expect("queue");
    q.enqueue("message-1", "bob@test").expect("enqueue");
    assert_eq!(
        Queue::open(&d).expect("reopen").pending().expect("pending"),
        vec![Delivery {
            id: "message-1".into(),
            recipient: "bob@test".into(),
            state: State::Pending
        }]
    );
    let _ = fs::remove_dir_all(d);
}

#[test]
fn latest_transition_wins_after_reopen() {
    let d = std::env::temp_dir().join("litterkeep-queue-transition-test");
    let q = Queue::open(&d).expect("queue");
    q.enqueue("message-2", "bob@test").expect("enqueue");
    q.transition("message-2", "bob@test", State::Delivered)
        .expect("transition");
    assert!(
        Queue::open(&d)
            .expect("reopen")
            .pending()
            .expect("pending")
            .is_empty()
    );
    let _ = fs::remove_dir_all(d);
}

#[test]
fn owned_delivery_is_invisible_until_store_commit() {
    let d = std::env::temp_dir().join("litterkeep-owned-queue-test");
    let q = Queue::open(&d).expect("queue");
    q.enqueue_owned(7, "alice@test", "remote@test", None)
        .expect("enqueue");
    let committed = HashSet::new();
    assert!(q.pending_owned(&committed).expect("uncommitted").is_empty());
    let mut committed = HashSet::new();
    committed.insert(7);
    assert_eq!(
        Queue::open(&d)
            .expect("reopen")
            .pending_owned(&committed)
            .expect("pending"),
        vec![OwnedDelivery {
            transaction_id: 7,
            attempts: 0,
            next_attempt_epoch: 0,
            sender: "alice@test".into(),
            recipient: "remote@test".into(),
            original_recipient_index: None,
            dsn_metadata: None,
            dsn_failure: None,
            state: State::Pending
        }]
    );
    let _ = fs::remove_dir_all(d);
}

/// Разные исходные RCPT не схлопываются при одинаковом redirect-адресе.
#[test]
fn owned_queue_recovery_preserves_distinct_original_recipient_indices() {
    let directory = std::env::temp_dir().join("litterkeep-owned-queue-origin-index-test");
    let queue = Queue::open(&directory).expect("queue");
    queue
        .enqueue_owned(17, "alice@test", "remote@test", Some(0))
        .expect("first route");
    queue
        .enqueue_owned(17, "alice@test", "remote@test", Some(1))
        .expect("second route");
    let committed = HashSet::from([17]);

    let recovered = Queue::open(&directory)
        .expect("reopen")
        .pending_owned(&committed)
        .expect("pending after recovery");

    assert_eq!(recovered.len(), 2);
    assert_eq!(
        recovered
            .first()
            .and_then(|job| job.original_recipient_index),
        Some(0)
    );
    assert_eq!(
        recovered
            .get(1)
            .and_then(|job| job.original_recipient_index),
        Some(1)
    );
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn owned_transition_survives_reopen_and_removes_pending_job() {
    let d = std::env::temp_dir().join("litterkeep-owned-transition-test");
    let q = Queue::open(&d).expect("queue");
    let delivery = OwnedDelivery {
        transaction_id: 8,
        attempts: 0,
        next_attempt_epoch: 0,
        sender: "alice@test".into(),
        recipient: "remote@test".into(),
        original_recipient_index: None,
        dsn_metadata: None,
        dsn_failure: None,
        state: State::Pending,
    };
    q.enqueue_owned(
        delivery.transaction_id,
        &delivery.sender,
        &delivery.recipient,
        delivery.original_recipient_index,
    )
    .expect("enqueue");
    q.transition_owned(&delivery, State::Delivered)
        .expect("transition");
    let mut committed = HashSet::new();
    committed.insert(delivery.transaction_id);
    assert!(
        Queue::open(&d)
            .expect("reopen")
            .pending_owned(&committed)
            .expect("pending")
            .is_empty()
    );
    let _ = fs::remove_dir_all(d);
}

#[test]
fn interrupted_owned_delivery_is_deferred_on_reopen() {
    let d = std::env::temp_dir().join("litterkeep-owned-inflight-test");
    let q = Queue::open(&d).expect("queue");
    let delivery = OwnedDelivery {
        transaction_id: 9,
        attempts: 0,
        next_attempt_epoch: 0,
        sender: "alice@test".into(),
        recipient: "remote@test".into(),
        original_recipient_index: None,
        dsn_metadata: None,
        dsn_failure: None,
        state: State::Pending,
    };
    q.enqueue_owned(
        delivery.transaction_id,
        &delivery.sender,
        &delivery.recipient,
        delivery.original_recipient_index,
    )
    .expect("enqueue");
    q.transition_owned(&delivery, State::InFlight)
        .expect("in flight");
    let mut committed = HashSet::new();
    committed.insert(delivery.transaction_id);
    assert_eq!(
        Queue::open(&d)
            .expect("reopen")
            .pending_owned(&committed)
            .expect("pending")
            .into_iter()
            .next()
            .expect("deferred")
            .state,
        State::Deferred
    );
    let _ = fs::remove_dir_all(d);
}

/// Терминальный failure notice переживает reopen и не ждёт retry deadline.
#[test]
fn terminal_failure_notice_is_recovered_as_due_work() {
    let directory = std::env::temp_dir().join("litterkeep-owned-dsn-notice-test");
    let queue = Queue::open(&directory).expect("queue");
    let mut delivery = OwnedDelivery {
        transaction_id: 31,
        attempts: 1,
        next_attempt_epoch: 0,
        sender: "alice@test".into(),
        recipient: "remote@test".into(),
        original_recipient_index: Some(0),
        dsn_metadata: None,
        dsn_failure: Some(crate::dsn::Failure::PermanentSmtp),
        state: State::PermanentFailure,
    };
    queue
        .enqueue_owned(
            delivery.transaction_id,
            &delivery.sender,
            &delivery.recipient,
            delivery.original_recipient_index,
        )
        .expect("enqueue");
    queue
        .transition_owned(&delivery, State::PermanentFailure)
        .expect("persist notice with terminal state");
    let committed = HashSet::from([delivery.transaction_id]);
    let recovered = Queue::open(&directory)
        .expect("reopen")
        .due_owned(&committed, u64::MAX)
        .expect("due terminal notice");
    assert_eq!(recovered, vec![delivery.clone()]);

    delivery.dsn_failure = None;
    Queue::open(&directory)
        .expect("reopen for acknowledgement")
        .transition_owned(&delivery, State::PermanentFailure)
        .expect("acknowledge notice");
    assert!(
        Queue::open(&directory)
            .expect("reopen after acknowledgement")
            .due_owned(&committed, u64::MAX)
            .expect("due after acknowledgement")
            .is_empty()
    );
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn deferred_epoch_survives_reopen_and_gates_due_selection() {
    let d = std::env::temp_dir().join("litterkeep-owned-retry-deadline-test");
    let q = Queue::open(&d).expect("queue");
    let delivery = OwnedDelivery {
        transaction_id: 10,
        attempts: 1,
        next_attempt_epoch: 1_060,
        sender: "alice@test".into(),
        recipient: "remote@test".into(),
        original_recipient_index: None,
        dsn_metadata: None,
        dsn_failure: None,
        state: State::Deferred,
    };
    q.enqueue_owned(
        delivery.transaction_id,
        &delivery.sender,
        &delivery.recipient,
        delivery.original_recipient_index,
    )
    .expect("enqueue");
    q.transition_owned(&delivery, State::Deferred)
        .expect("defer");
    let committed = HashSet::from([delivery.transaction_id]);
    let reopened = Queue::open(&d).expect("reopen");
    assert!(
        reopened
            .due_owned(&committed, 1_059)
            .expect("before deadline")
            .is_empty()
    );
    assert_eq!(
        reopened.due_owned(&committed, 1_060).expect("at deadline"),
        vec![delivery]
    );
    let _ = fs::remove_dir_all(d);
}

#[test]
fn previous_attempts_frame_defaults_to_immediate_retry() {
    let d = std::env::temp_dir().join("litterkeep-owned-queue-v2-test");
    let queue_dir = d.join("queue");
    fs::create_dir_all(&queue_dir).expect("queue directory");
    fs::write(
        queue_dir.join("delivery.queue"),
        "deferred 11 2 alice@test remote@test\n",
    )
    .expect("previous frame");
    let committed = HashSet::from([11]);
    let delivery = Queue::open(&queue_dir)
        .expect("open")
        .due_owned(&committed, 0)
        .expect("due")
        .into_iter()
        .next()
        .expect("delivery");
    assert_eq!(delivery.attempts, 2);
    assert_eq!(delivery.next_attempt_epoch, 0);
    let _ = fs::remove_dir_all(d);
}
