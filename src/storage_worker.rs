//! The single owner of a mutable Store, fed by a bounded request queue.
use crate::store::{Message, Store};
use std::io;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::thread::{self, JoinHandle};

#[derive(Debug)]
pub enum Request {
    Append {
        request_id: u64,
        mailbox: String,
        bytes: Vec<u8>,
    },
    Read {
        request_id: u64,
        mailbox: String,
    },
}
#[derive(Debug)]
pub enum Response {
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
        let store = Store::open(root)?;
        let (requests, incoming) = mpsc::sync_channel(capacity);
        let (outgoing, responses) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("litterkeep-store-writer".into())
            .spawn(move || run(store, incoming, outgoing))?;
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

fn run(store: Store, requests: Receiver<Request>, responses: mpsc::Sender<Response>) {
    for request in requests {
        let response = match request {
            Request::Append {
                request_id,
                mailbox,
                bytes,
            } => Response::Appended {
                request_id,
                result: store.append(&mailbox, &bytes),
            },
            Request::Read {
                request_id,
                mailbox,
            } => Response::Read {
                request_id,
                result: store.messages(&mailbox),
            },
        };
        if responses.send(response).is_err() {
            break;
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
}
