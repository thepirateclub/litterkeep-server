//! Сквозная проверка SMTP DSN request через единственного Store owner.

use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

/// DSN policy из bounded request переживает owner completion и reopen.
#[test]
fn append_many_persists_typed_smtp_dsn_after_owner_completion() {
    let root = std::env::temp_dir().join(format!(
        "litterkeep-worker-dsn-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let accounts = Accounts::open(&root).expect("accounts");
    accounts.create("bob", b"test-password").expect("account");
    let expected = crate::smtp::DsnEnvelope {
        reverse_path: "alice@test".into(),
        mail: crate::smtp::MailDsn {
            return_content: Some(crate::smtp::ReturnContent::Headers),
            envelope_id: Some(b"worker-transaction".to_vec()),
        },
        recipients: vec![crate::smtp::RcptTo {
            address: "bob@test".into(),
            dsn: crate::smtp::RecipientDsn {
                notify: Some(crate::smtp::NotifyPolicy::Conditions(
                    [crate::smtp::NotifyCondition::Failure]
                        .into_iter()
                        .collect(),
                )),
                original_recipient: None,
            },
        }],
    };
    let worker = StorageWorker::start(&root, 1).expect("worker");
    worker
        .submit(Request::AppendMany(Box::new(SmtpDeliveryRequest {
            request_id: 9,
            mailboxes: vec!["bob".into()],
            envelope_recipients: vec!["bob@test".into()],
            envelope_sender: "alice@test".into(),
            dsn: expected.clone(),
            bytes: b"message".to_vec(),
        })))
        .expect("accepted");
    receive_successful_append(&worker);
    worker.shutdown();
    let store = Store::open(&root).expect("reopen store");
    assert!(store.contains_smtp_dsn_envelope(&expected));
    drop(store);
    std::fs::remove_dir_all(root).expect("cleanup");
}

/// Ожидает completion bounded request с конечным сроком теста.
fn receive_successful_append(worker: &StorageWorker) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match worker.try_response() {
            Ok(Response::Appended {
                request_id: 9,
                result: Ok(_),
            }) => return,
            Ok(response) => panic!("unexpected storage response: {response:?}"),
            Err(TryRecvError::Empty) if std::time::Instant::now() < deadline => {
                std::thread::yield_now();
            }
            Err(error) => panic!("storage completion was not produced: {error}"),
        }
    }
}
