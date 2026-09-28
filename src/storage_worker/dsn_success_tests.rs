//! End-to-end проверки Store-owner consumer-а delivered DSN.

use super::*;
use crate::{
    queue::DsnSuccessKey,
    smtp::{DsnEnvelope, MailDsn, NotifyCondition, NotifyPolicy, RcptTo, RecipientDsn},
};
use std::time::{SystemTime, UNIX_EPOCH};

fn root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-worker-success-dsn-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

fn envelope() -> DsnEnvelope {
    DsnEnvelope {
        reverse_path: "alice@litterkeep.test".into(),
        mail: MailDsn::default(),
        recipients: vec![RcptTo {
            address: "bob@litterkeep.test".into(),
            dsn: RecipientDsn {
                notify: Some(NotifyPolicy::Conditions(
                    [NotifyCondition::Success].into_iter().collect(),
                )),
                original_recipient: None,
            },
        }],
    }
}

#[test]
fn consumer_publishes_one_delivered_report_and_recovers_acknowledgement() {
    let directory = root();
    let accounts = Accounts::open(&directory).expect("open accounts");
    accounts
        .create("alice", b"test-password")
        .expect("alice account");
    let source = b"Subject: original\r\n\r\nattachment bytes";
    let request = envelope();
    let source_id = {
        let mut store = Store::open(&directory).expect("open Store");
        store
            .deliver_sieve_many_with_dsn(
                &["bob".into()],
                &request.reverse_path,
                &["bob@litterkeep.test".into()],
                source,
                Some(&request),
            )
            .expect("commit source mailbox delivery")
    };
    let key = DsnSuccessKey {
        transaction_id: source_id,
        recipient_index: 0,
    };
    let first_worker = StorageWorker::start(&directory, 2).expect("start worker");
    first_worker
        .submit(Request::Read {
            request_id: 1,
            mailbox: "alice".into(),
        })
        .expect("wait for startup sweep");
    assert!(matches!(
        receive(&first_worker),
        Response::Read {
            request_id: 1,
            result: Ok(snapshot)
        } if snapshot.messages.len() == 1
    ));
    first_worker.shutdown();

    let store = Store::open(&directory).expect("verify automatic report");
    assert!(store.pending_dsn_success_notices().is_empty());
    let automatic_reports = store.messages("alice").expect("alice mailbox");
    let automatic_report = automatic_reports.first().expect("startup delivered report");
    assert!(
        automatic_report
            .bytes
            .windows(b"Action: delivered".len())
            .any(|value| value == b"Action: delivered")
    );
    drop(store);

    let recovered_worker = StorageWorker::start(&directory, 2).expect("restart worker");
    recovered_worker
        .submit(Request::Dsn(DsnRequest::Success { request_id: 2, key }))
        .expect("retry intent through bounded worker");
    assert!(matches!(
        receive(&recovered_worker),
        Response::Dsn(DsnResponse::Success {
            request_id: 2,
            result: Ok(Some(transaction_id))
        }) if transaction_id > key.transaction_id
    ));
    recovered_worker.shutdown();

    let store = Store::open(&directory).expect("recover report Store");
    assert!(store.pending_dsn_success_notices().is_empty());
    let reports = store.messages("alice").expect("alice mailbox");
    assert_eq!(reports.len(), 1);
    let report = reports.first().expect("one delivered report");
    assert!(
        report
            .bytes
            .windows(b"Action: delivered".len())
            .any(|value| value == b"Action: delivered")
    );
    assert!(
        report
            .bytes
            .windows(b"Subject: original".len())
            .any(|value| value == b"Subject: original")
    );
    assert!(
        !report
            .bytes
            .windows(b"attachment bytes".len())
            .any(|value| value == b"attachment bytes")
    );
    drop(store);
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn external_sender_receives_report_as_null_reverse_path_outbound_job() {
    let directory = root();
    let accounts = Accounts::open(&directory).expect("open accounts");
    accounts
        .create("bob", b"test-password")
        .expect("bob account");
    let mut request = envelope();
    request.reverse_path = "sender@outside.example".into();
    let source = b"Subject: external receipt\r\n\r\nprivate body";
    {
        let mut store = Store::open(&directory).expect("open Store");
        store
            .deliver_sieve_many_with_dsn(
                &["bob".into()],
                &request.reverse_path,
                &["bob@litterkeep.test".into()],
                source,
                Some(&request),
            )
            .expect("commit local delivery");
    }

    let worker = StorageWorker::start(&directory, 2).expect("start automatic consumer");
    worker
        .submit(Request::OutboundPending { request_id: 3 })
        .expect("query outbound after startup sweep");
    let deliveries = match receive(&worker) {
        Response::OutboundPending {
            request_id: 3,
            result,
        } => result.expect("outbound jobs"),
        _ => panic!("unexpected outbound response"),
    };
    worker.shutdown();

    let store = Store::open(&directory).expect("reopen Store");
    assert!(store.pending_dsn_success_notices().is_empty());
    assert_eq!(deliveries.len(), 1);
    let delivery = deliveries.first().expect("one external DSN job");
    assert!(delivery.sender.is_empty());
    assert_eq!(delivery.recipient, "sender@outside.example");
    let report = store.outbound_blob(delivery).expect("report blob");
    assert!(
        report
            .windows(b"Action: delivered".len())
            .any(|value| value == b"Action: delivered")
    );
    assert!(
        report
            .windows(b"Subject: external receipt".len())
            .any(|value| value == b"Subject: external receipt")
    );
    assert!(
        !report
            .windows(b"private body".len())
            .any(|value| value == b"private body")
    );
    assert_eq!(store.pending_outbound().expect("recovered queue").len(), 1);
    drop(store);
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn idle_worker_continues_success_notice_backlog_beyond_one_bounded_page() {
    let directory = root();
    let accounts = Accounts::open(&directory).expect("open accounts");
    accounts
        .create("alice", b"test-password")
        .expect("alice account");
    let request = envelope();
    {
        let mut store = Store::open(&directory).expect("open Store");
        for sequence in 0..101 {
            let message = format!("Subject: receipt {sequence}\r\n\r\nbody");
            store
                .deliver_sieve_many_with_dsn(
                    &["bob".into()],
                    &request.reverse_path,
                    &["bob@litterkeep.test".into()],
                    message.as_bytes(),
                    Some(&request),
                )
                .expect("commit source delivery");
        }
        assert_eq!(store.pending_dsn_success_notices().len(), 101);
    }

    let worker = StorageWorker::start(&directory, 2).expect("start owner");
    std::thread::sleep(std::time::Duration::from_secs(3));
    worker
        .submit(Request::Read {
            request_id: 101,
            mailbox: "alice".into(),
        })
        .expect("read after idle background sweeps");
    let response = receive(&worker);
    assert!(matches!(
        response,
        Response::Read {
            request_id: 101,
            result: Ok(snapshot)
        } if snapshot.messages.len() == 101
    ));
    worker.shutdown();

    let recovered = Store::open(&directory).expect("verify fully swept backlog");
    assert!(recovered.pending_dsn_success_notices().is_empty());
    assert_eq!(recovered.messages("alice").expect("reports").len(), 101);
    drop(recovered);
    std::fs::remove_dir_all(directory).expect("cleanup");
}

fn receive(worker: &StorageWorker) -> Response {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Ok(response) = worker.try_response() {
            return response;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "worker response timeout"
        );
        std::thread::yield_now();
    }
}
