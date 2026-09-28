//! Жизненный цикл единственного владельца очереди StorageWorker.

use super::{Request, Response, dsn_delivery, execution};
use crate::{accounts::Accounts, store::Store};
use std::sync::mpsc::{Receiver, Sender};

/// Последовательно обслуживает запросы и завершает поток после закрытия канала.
pub(super) fn run(
    mut store: Store,
    accounts: Accounts,
    requests: Receiver<Request>,
    responses: Sender<Response>,
) {
    report_pending_success_error(dsn_delivery::append_pending_successes(
        &mut store, &accounts,
    ));
    for request in requests {
        let response = execution::respond(&mut store, &accounts, request);
        if responses.send(response).is_err() {
            break;
        }
        report_pending_success_error(dsn_delivery::append_pending_successes(
            &mut store, &accounts,
        ));
    }
}

/// Не теряет source delivery из-за отчёта, но оставляет ошибку видимой.
fn report_pending_success_error(result: std::io::Result<usize>) {
    if let Err(error) = result {
        eprintln!("Local SUCCESS DSN sweep failed; intents remain pending: {error}");
    }
}
