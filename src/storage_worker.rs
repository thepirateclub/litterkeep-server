//! The single owner of a mutable Store, fed by a bounded request queue.
#[path = "storage_worker/dsn_delivery.rs"]
mod dsn_delivery;
#[path = "storage_worker/execution.rs"]
mod execution;
#[path = "storage_worker/imap_append.rs"]
mod imap_append;
#[path = "storage_worker/local_delivery.rs"]
mod local_delivery;
#[path = "storage_worker/message.rs"]
mod message;
#[path = "storage_worker/owner.rs"]
mod owner;
#[path = "storage_worker/sasl_lookup.rs"]
mod sasl_lookup;
#[cfg(test)]
#[path = "storage_worker/sasl_lookup_tests.rs"]
mod sasl_lookup_tests;
#[cfg(test)]
#[path = "storage_worker/smtp_dsn_tests.rs"]
mod smtp_dsn_tests;
mod start;
mod submit;
#[cfg(target_os = "linux")]
pub(crate) use crate::{accounts::Accounts, store::Store};
#[cfg(test)]
#[cfg(not(target_os = "linux"))]
pub(crate) use crate::{accounts::Accounts, store::Store};
pub use imap_append::ImapAppendResult;
pub use message::{Request, Response, SmtpDeliveryRequest};
pub use sasl_lookup::{
    ScramSha256VerifierRequest, ScramSha256VerifierResponse, ScramVerifierRequest,
    ScramVerifierResponse,
};
#[cfg(target_os = "linux")]
use std::io;
#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
#[cfg(target_os = "linux")]
use std::thread;
use std::thread::JoinHandle;
pub use submit::SubmitError;

pub struct StorageWorker {
    requests: SyncSender<Request>,
    responses: Receiver<Response>,
    thread: JoinHandle<()>,
}

impl StorageWorker {
    pub fn try_response(&self) -> Result<Response, TryRecvError> {
        self.responses.try_recv()
    }
    pub fn shutdown(self) {
        drop(self.requests);
        let _ = self.thread.join();
    }
}

/// Linux reactor integration. The byte is only a coalesced wakeup hint; the
/// authoritative completion remains in the channel, so a full wake socket
/// cannot lose ownership or completion transitions.
#[cfg(target_os = "linux")]
pub struct WakingStorageWorker {
    pub worker: StorageWorker,
    wake_reader: std::os::unix::net::UnixStream,
}

#[cfg(target_os = "linux")]
impl WakingStorageWorker {
    pub fn start(root: impl AsRef<Path>, capacity: usize) -> io::Result<Self> {
        use std::io::Write;
        use std::os::unix::net::UnixStream;
        let mut store = Store::open(root.as_ref())?;
        let accounts = Accounts::open(root.as_ref())?;
        let (requests, incoming) = mpsc::sync_channel(capacity);
        let (outgoing, responses) = mpsc::channel();
        let (wake_reader, mut wake_writer) = UnixStream::pair()?;
        wake_reader.set_nonblocking(true)?;
        wake_writer.set_nonblocking(true)?;
        let thread = thread::Builder::new()
            .name("litterkeep-store-writer".into())
            .spawn(move || {
                for request in incoming {
                    let response = execution::respond(&mut store, &accounts, request);
                    if outgoing.send(response).is_err() {
                        break;
                    }
                    let _ = wake_writer.write(&[1]);
                }
            })?;
        Ok(Self {
            worker: StorageWorker {
                requests,
                responses,
                thread,
            },
            wake_reader,
        })
    }
    pub fn wake_reader(&self) -> &std::os::unix::net::UnixStream {
        &self.wake_reader
    }
    pub fn drain_wakeup(&mut self) -> io::Result<()> {
        use std::io::Read;
        let mut bytes = [0_u8; 256];
        loop {
            match self.wake_reader.read(&mut bytes) {
                Ok(0) => return Ok(()),
                Ok(_) => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) => return Err(error),
            }
        }
    }

    /// Останавливает Store owner до уничтожения wake descriptor-а.
    pub fn shutdown(self) {
        self.worker.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    fn dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "litterkeep-worker-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }
    #[test]
    fn accepted_write_completes_after_requester_has_no_store_lease() {
        let root = dir();
        let worker = StorageWorker::start(&root, 1).expect("worker");
        worker
            .submit(Request::Append {
                request_id: 7,
                mailbox: "bob".into(),
                bytes: b"message".to_vec(),
                metadata: crate::store::AppendMetadata::default(),
            })
            .expect("accepted");
        let response = loop {
            if let Ok(response) = worker.try_response() {
                break response;
            }
            std::thread::yield_now();
        };
        assert!(matches!(
            response,
            Response::Appended {
                request_id: 7,
                result: Ok(1)
            }
        ));
        worker.shutdown();
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn append_many_rejects_unknown_recipient_before_publication() {
        let root = dir();
        let accounts = Accounts::open(&root).expect("accounts");
        accounts.create("bob", b"test-password").expect("account");
        let worker = StorageWorker::start(&root, 1).expect("worker");
        worker
            .submit(Request::AppendMany(Box::new(SmtpDeliveryRequest {
                request_id: 8,
                mailboxes: vec!["bob".into(), "unknown".into()],
                envelope_recipients: vec!["bob@test".into(), "unknown@test".into()],
                envelope_sender: "alice@test".into(),
                dsn: crate::smtp::DsnEnvelope {
                    reverse_path: "alice@test".into(),
                    mail: Default::default(),
                    recipients: vec![
                        crate::smtp::RcptTo {
                            address: "bob@test".into(),
                            dsn: Default::default(),
                        },
                        crate::smtp::RcptTo {
                            address: "unknown@test".into(),
                            dsn: Default::default(),
                        },
                    ],
                },
                bytes: b"message".to_vec(),
            })))
            .expect("accepted");
        let response = loop {
            if let Ok(response) = worker.try_response() {
                break response;
            }
            std::thread::yield_now();
        };
        assert!(matches!(
            response,
            Response::Appended {
                request_id: 8,
                result: Err(_)
            }
        ));
        worker.shutdown();
        let store = Store::open(&root).expect("reopen");
        assert!(store.messages("bob").expect("messages").is_empty());
        drop(store);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn dsn_is_published_only_for_existing_local_sender() {
        let root = dir();
        let accounts = Accounts::open(&root).expect("accounts");
        accounts.create("alice", b"test-password").expect("account");
        let worker = StorageWorker::start(&root, 1).expect("worker");
        worker
            .submit(Request::AppendDsn {
                request_id: 9,
                reverse_path: "alice@litterkeep.test".into(),
                recipient: "remote@peer.test".into(),
                transaction_id: 42,
                original: b"Subject: source\r\n\r\nbody".to_vec(),
                failure: crate::dsn::Failure::PermanentSmtp,
                dsn_metadata: None,
                report_key: crate::queue::DsnReportKey {
                    transaction_id: 42,
                    recipient: "remote@peer.test".into(),
                    original_recipient_index: None,
                    failure: crate::dsn::Failure::PermanentSmtp,
                },
            })
            .expect("accepted");
        let response = loop {
            if let Ok(response) = worker.try_response() {
                break response;
            }
            std::thread::yield_now();
        };
        assert!(matches!(
            response,
            Response::DsnAppended {
                request_id: 9,
                result: Ok(Some(_))
            }
        ));
        worker.shutdown();
        let store = Store::open(&root).expect("reopen");
        let messages = store.messages("alice").expect("messages");
        assert_eq!(messages.len(), 1);
        assert!(
            messages
                .first()
                .expect("dsn message")
                .bytes
                .windows(b"Action: failed".len())
                .any(|window| window == b"Action: failed")
        );
        drop(store);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn sieve_put_is_completed_by_the_single_store_owner() {
        let root = dir();
        let accounts = Accounts::open(&root).expect("accounts");
        accounts.create("bob", b"test-password").expect("account");
        let worker = StorageWorker::start(&root, 1).expect("worker");
        worker
            .submit(Request::SievePut {
                request_id: 9,
                account: "bob".into(),
                name: "invoices".into(),
                source: "require [\"fileinto\"]; fileinto \"Invoices\";".into(),
            })
            .expect("accepted");
        let response = loop {
            if let Ok(response) = worker.try_response() {
                break response;
            }
            std::thread::yield_now();
        };
        assert!(matches!(
            response,
            Response::SieveStored {
                request_id: 9,
                result: Ok(_)
            }
        ));
        worker.shutdown();
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
