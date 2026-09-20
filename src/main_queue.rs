//! CLI boundary одного bounded запуска долговечной исходящей очереди.

use crate::usage;
use std::env;
use std::process::ExitCode;

/// Выполняет одну bounded пачку durable outbound jobs через собственный SMTP.
pub(crate) fn drain() -> ExitCode {
    #[cfg(target_os = "linux")]
    return drain_linux();
    #[cfg(not(target_os = "linux"))]
    drain_portable()
}

/// Linux путь использует wake-driven владельцев Store и SMTP pool.
#[cfg(target_os = "linux")]
fn drain_linux() -> ExitCode {
    let (data, peer, limit) = match arguments() {
        Ok(value) => value,
        Err(code) => return code,
    };
    match litterkeep_server::outbound_service::drain(&data, &peer, limit) {
        Ok(report) => {
            println!(
                "delivered={} deferred={} permanent_failures={}",
                report.delivered, report.deferred, report.permanent_failures
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("queue-drain error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Остаётся ограниченным fallback для платформ без Linux wake socket API.
#[cfg(not(target_os = "linux"))]
fn drain_portable() -> ExitCode {
    let (data, peer, limit) = match arguments() {
        Ok(value) => value,
        Err(code) => return code,
    };
    let mut store = match litterkeep_server::store::Store::open(&data) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("queue-drain store error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let accounts = match litterkeep_server::accounts::Accounts::open(&data) {
        Ok(accounts) => accounts,
        Err(error) => {
            eprintln!("queue-drain accounts error: {error}");
            return ExitCode::FAILURE;
        }
    };
    match litterkeep_server::outbound_queue::drain(&mut store, &accounts, &peer, limit) {
        Ok(report) => {
            println!(
                "delivered={} deferred={} permanent_failures={}",
                report.delivered, report.deferred, report.permanent_failures
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("queue-drain error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Разбирает CLI только один раз для platform-specific service boundary.
fn arguments() -> Result<(String, String, usize), ExitCode> {
    let mut arguments = env::args().skip(2);
    let Some(data) = arguments.next() else {
        usage();
        return Err(ExitCode::from(2));
    };
    let Some(peer) = arguments.next() else {
        usage();
        return Err(ExitCode::from(2));
    };
    let limit = match arguments.next() {
        Some(value) => match value.parse::<usize>() {
            Ok(limit) if limit > 0 => limit,
            Ok(_) | Err(_) => {
                eprintln!("queue-drain limit must be positive");
                return Err(ExitCode::from(2));
            }
        },
        None => 8,
    };
    if arguments.next().is_some() {
        usage();
        return Err(ExitCode::from(2));
    }
    Ok((data, peer, limit))
}
