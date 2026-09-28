//! Жизненный цикл единственного владельца очереди StorageWorker.

use super::{Request, Response, dsn_delivery, execution};
use crate::{accounts::Accounts, store::Store};
use std::sync::mpsc::{Receiver, Sender};
#[cfg(target_os = "linux")]
use std::{io::Write, os::unix::net::UnixStream};

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

/// Обслуживает Linux reactor-worker с тем же recovery sweep и wake сигналом.
#[cfg(target_os = "linux")]
pub(super) fn run_waking(
    mut store: Store,
    accounts: Accounts,
    requests: Receiver<Request>,
    responses: Sender<Response>,
    mut wake_writer: UnixStream,
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
        signal_reactor(&mut wake_writer);
    }
}

/// Сохраняет coalesced wake semantics и не скрывает ошибки кроме WouldBlock.
#[cfg(target_os = "linux")]
fn signal_reactor(writer: &mut UnixStream) {
    match writer.write(&[1_u8]) {
        Ok(1) => {}
        Ok(_) => eprintln!("StorageWorker wake signal wrote no byte"),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
        Err(error) => eprintln!("StorageWorker wake signal failed: {error}"),
    }
}
