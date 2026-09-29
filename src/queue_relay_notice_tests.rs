//! Проверяет recovery relay DSN notice из durable очереди.

use super::{OwnedDelivery, Queue, State};
use std::{collections::HashSet, fs};

/// Relay notice остаётся due после restart даже при далёком retry deadline.
#[test]
fn relayed_notice_is_recovered_as_due_work() {
    let directory = std::env::temp_dir().join("litterkeep-owned-relayed-notice-test");
    let queue = Queue::open(&directory).expect("queue");
    let delivery = OwnedDelivery {
        transaction_id: 32,
        attempts: 1,
        next_attempt_epoch: u64::MAX,
        sender: "alice@test".into(),
        recipient: "remote@test".into(),
        original_recipient_indices: vec![0],
        dsn_metadata: None,
        dsn_notice: Some(crate::dsn::Notice::Relayed),
        dsn_diagnostic: None,
        state: State::Delivered,
    };
    queue
        .enqueue_owned_many(
            delivery.transaction_id,
            &delivery.sender,
            &delivery.recipient,
            delivery.original_recipient_indices.as_slice(),
        )
        .expect("enqueue");
    queue
        .transition_owned(&delivery, State::Delivered)
        .expect("persist relay notice");
    let committed = HashSet::from([delivery.transaction_id]);

    let recovered = Queue::open(&directory)
        .expect("reopen")
        .due_owned(&committed, 1)
        .expect("due after restart");

    assert_eq!(recovered, vec![delivery]);
    let _ = fs::remove_dir_all(directory);
}

/// Отложенный DSN notice сохраняется и восстанавливается как typed work.
#[test]
fn delayed_notice_is_recovered_as_due_work() {
    let directory = std::env::temp_dir().join("litterkeep-owned-delayed-notice-test");
    let queue = Queue::open(&directory).expect("queue");
    let mut delivery = OwnedDelivery {
        transaction_id: 33,
        attempts: 2,
        next_attempt_epoch: u64::MAX,
        sender: "alice@test".into(),
        recipient: "remote@test".into(),
        original_recipient_indices: vec![0],
        dsn_metadata: None,
        dsn_notice: Some(crate::dsn::Notice::Delayed),
        dsn_diagnostic: None,
        state: State::Deferred,
    };
    queue
        .enqueue_owned_many(
            delivery.transaction_id,
            &delivery.sender,
            &delivery.recipient,
            delivery.original_recipient_indices.as_slice(),
        )
        .expect("enqueue");
    queue
        .transition_owned(&delivery, State::Deferred)
        .expect("persist delayed notice");
    let committed = HashSet::from([delivery.transaction_id]);

    let recovered = Queue::open(&directory)
        .expect("reopen")
        .due_owned(&committed, 1)
        .expect("due after restart");

    delivery.dsn_metadata = None;
    assert_eq!(recovered, vec![delivery]);
    let _ = fs::remove_dir_all(directory);
}
