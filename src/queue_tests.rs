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
    q.enqueue_owned(7, "alice@test", "remote@test")
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
            state: State::Pending
        }]
    );
    let _ = fs::remove_dir_all(d);
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
        state: State::Pending,
    };
    q.enqueue_owned(
        delivery.transaction_id,
        &delivery.sender,
        &delivery.recipient,
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
        state: State::Pending,
    };
    q.enqueue_owned(
        delivery.transaction_id,
        &delivery.sender,
        &delivery.recipient,
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
        state: State::Deferred,
    };
    q.enqueue_owned(
        delivery.transaction_id,
        &delivery.sender,
        &delivery.recipient,
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
