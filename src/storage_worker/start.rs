//! Создание Store owner и его bounded каналов.

use super::{StorageWorker, owner};
use crate::{accounts::Accounts, store::Store};
use std::{io, path::Path, sync::mpsc, thread};

impl StorageWorker {
    /// Создаёт единственного владельца Store и ограниченный канал команд.
    pub fn start(root: impl AsRef<Path>, capacity: usize) -> io::Result<Self> {
        let store = Store::open(root.as_ref())?;
        let accounts = Accounts::open(root.as_ref())?;
        let (requests, incoming) = mpsc::sync_channel(capacity);
        let (outgoing, responses) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("litterkeep-store-writer".into())
            .spawn(move || owner::run(store, accounts, incoming, outgoing))?;
        Ok(Self {
            requests,
            responses,
            thread,
        })
    }
}
