//! Жизненный цикл единственного владельца очереди StorageWorker.

use super::{Request, Response, dsn_delivery, execution};
use crate::{accounts::Accounts, store::Store};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;
#[cfg(target_os = "linux")]
use std::{io::Write, os::unix::net::UnixStream};

/// Последовательно обслуживает запросы и завершает поток после закрытия канала.
pub(super) fn run(
    mut store: Store,
    accounts: Accounts,
    requests: Receiver<Request>,
    responses: Sender<Response>,
) {
    let mut success_cursor = None;
    sweep_pending_successes(&mut store, &accounts, &mut success_cursor);
    loop {
        match requests.recv_timeout(Duration::from_millis(100)) {
            Ok(request) => {
                let response = execution::respond(&mut store, &accounts, request);
                if responses.send(response).is_err() {
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        sweep_pending_successes(&mut store, &accounts, &mut success_cursor);
    }
}

/// Выполняет одну ограниченную порцию и сохраняет ошибку для следующего прохода.
fn sweep_pending_successes(
    store: &mut Store,
    accounts: &Accounts,
    cursor: &mut Option<crate::queue::DsnSuccessKey>,
) {
    if let Err(error) = dsn_delivery::append_pending_successes(store, accounts, cursor) {
        eprintln!("SUCCESS DSN sweep failed; intents remain pending: {error}");
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
    let mut success_cursor = None;
    sweep_pending_successes(&mut store, &accounts, &mut success_cursor);
    loop {
        let (published, request_processed) = match requests.recv_timeout(Duration::from_millis(100))
        {
            Ok(request) => {
                let response = execution::respond(&mut store, &accounts, request);
                if responses.send(response).is_err() {
                    break;
                }
                let published =
                    sweep_pending_successes_count(&mut store, &accounts, &mut success_cursor);
                (published, true)
            }
            Err(RecvTimeoutError::Timeout) => (
                sweep_pending_successes_count(&mut store, &accounts, &mut success_cursor),
                false,
            ),
            Err(RecvTimeoutError::Disconnected) => break,
        };
        if request_processed || published > 0 {
            signal_reactor(&mut wake_writer);
        }
    }
}

/// Возвращает число завершённых intents для управления reactor wakeup-ом.
#[cfg(target_os = "linux")]
fn sweep_pending_successes_count(
    store: &mut Store,
    accounts: &Accounts,
    cursor: &mut Option<crate::queue::DsnSuccessKey>,
) -> usize {
    match dsn_delivery::append_pending_successes(store, accounts, cursor) {
        Ok(count) => count,
        Err(error) => {
            eprintln!("SUCCESS DSN sweep failed; intents remain pending: {error}");
            0
        }
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
