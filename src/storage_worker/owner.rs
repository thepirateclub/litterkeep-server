//! Жизненный цикл единственного владельца очереди StorageWorker.

use super::{Request, Response, execution};
use crate::{accounts::Accounts, store::Store};
use std::sync::mpsc::{Receiver, Sender};

/// Последовательно обслуживает запросы и завершает поток после закрытия канала.
pub(super) fn run(
    mut store: Store,
    accounts: Accounts,
    requests: Receiver<Request>,
    responses: Sender<Response>,
) {
    for request in requests {
        let response = execution::respond(&mut store, &accounts, request);
        if responses.send(response).is_err() {
            break;
        }
    }
}
