//! The single owner of a mutable Store, fed by a bounded request queue.
#[path = "storage_worker/execution.rs"]
mod execution;
use crate::{
    accounts::Accounts,
    store::{Message, Store},
};
use std::io;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::thread::{self, JoinHandle};

#[derive(Debug)]
pub enum Request {
    /// Проверяет credential вне сетевого reactor.
    Authenticate {
        request_id: u64,
        username: String,
        password: Vec<u8>,
    },
    /// Проверяет local-part envelope до начала SMTP DATA.
    ValidateRecipient {
        request_id: u64,
        mailbox: String,
    },
    Append {
        request_id: u64,
        mailbox: String,
        bytes: Vec<u8>,
    },
    /// Долговечно сохраняет одно сообщение во все проверенные локальные inbox.
    AppendMany {
        request_id: u64,
        mailboxes: Vec<String>,
        bytes: Vec<u8>,
    },
    Read {
        request_id: u64,
        mailbox: String,
    },
}
#[derive(Debug)]
pub enum Response {
    /// Completion проверки аккаунта.
    Authenticated { request_id: u64, accepted: bool },
    /// Ответ policy-проверки локального SMTP-получателя.
    RecipientValidated { request_id: u64, accepted: bool },
    Appended {
        request_id: u64,
        result: io::Result<u64>,
    },
    Read {
        request_id: u64,
        result: io::Result<Vec<Message>>,
    },
}

pub struct StorageWorker {
    requests: SyncSender<Request>,
    responses: Receiver<Response>,
    thread: JoinHandle<()>,
}

impl StorageWorker {
    pub fn start(root: impl AsRef<Path>, capacity: usize) -> io::Result<Self> {
        let store = Store::open(root.as_ref())?;
        let accounts = Accounts::open(root.as_ref())?;
        let (requests, incoming) = mpsc::sync_channel(capacity);
        let (outgoing, responses) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("litterkeep-store-writer".into())
            .spawn(move || execution::run(store, accounts, incoming, outgoing))?;
        Ok(Self {
            requests,
            responses,
            thread,
        })
    }
    pub fn submit(&self, request: Request) -> Result<(), mpsc::TrySendError<Request>> {
        self.requests.try_send(request)
    }
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
        let store = Store::open(root.as_ref())?;
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
                    let response = execution::respond(&store, &accounts, request);
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
            .submit(Request::AppendMany {
                request_id: 8,
                mailboxes: vec!["bob".into(), "unknown".into()],
                bytes: b"message".to_vec(),
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
}
