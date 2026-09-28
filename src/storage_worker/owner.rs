//! Жизненный цикл единственного владельца очереди StorageWorker.

use super::retry_delay::RetryDelay;
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
    let mut retry = RetryDelay::new();
    sweep_pending_successes(&mut store, &accounts, &mut success_cursor, &mut retry);
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
        sweep_pending_successes(&mut store, &accounts, &mut success_cursor, &mut retry);
    }
}

/// Выполняет одну ограниченную порцию и сохраняет ошибку для следующего прохода.
fn sweep_pending_successes(
    store: &mut Store,
    accounts: &Accounts,
    cursor: &mut Option<crate::queue::DsnSuccessKey>,
    retry: &mut RetryDelay,
) {
    match run_success_sweep(store, accounts, cursor, retry) {
        Ok(_) => {}
        Err(error) => eprintln!("SUCCESS DSN sweep failed; intents remain pending: {error}"),
    }
}

/// Запускает только наступивший bounded проход и обновляет пакетную задержку.
fn run_success_sweep(
    store: &mut Store,
    accounts: &Accounts,
    cursor: &mut Option<crate::queue::DsnSuccessKey>,
    retry: &mut RetryDelay,
) -> Result<usize, String> {
    if !retry.is_ready(std::time::Instant::now()) {
        return Ok(0);
    }
    match dsn_delivery::append_pending_successes(store, accounts, cursor) {
        Ok(count) => {
            retry.succeeded();
            Ok(count)
        }
        Err(error) => {
            retry.failed(std::time::Instant::now());
            Err(error.to_string())
        }
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
    let mut retry = RetryDelay::new();
    sweep_pending_successes(&mut store, &accounts, &mut success_cursor, &mut retry);
    loop {
        let (published, request_processed) = match requests.recv_timeout(Duration::from_millis(100))
        {
            Ok(request) => {
                let response = execution::respond(&mut store, &accounts, request);
                if responses.send(response).is_err() {
                    break;
                }
                let published = sweep_pending_successes_count(
                    &mut store,
                    &accounts,
                    &mut success_cursor,
                    &mut retry,
                );
                (published, true)
            }
            Err(RecvTimeoutError::Timeout) => (
                sweep_pending_successes_count(
                    &mut store,
                    &accounts,
                    &mut success_cursor,
                    &mut retry,
                ),
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
    retry: &mut RetryDelay,
) -> usize {
    match run_success_sweep(store, accounts, cursor, retry) {
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
