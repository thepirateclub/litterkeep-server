//! CLI offline maintenance с явным `--data-dir` и диагностикой lock/error.

use std::env;
use std::process::ExitCode;

pub(crate) fn check() -> ExitCode {
    let mut arguments = env::args().skip(2);
    let data = match (arguments.next().as_deref(), arguments.next()) {
        (Some("--data-dir"), Some(path)) => path,
        _ => {
            super::usage();
            return ExitCode::from(2);
        }
    };
    if arguments.next().is_some() {
        super::usage();
        return ExitCode::from(2);
    }
    match litterkeep_server::maintenance::check(data) {
        Ok(report) => {
            println!(
                "CHECK ok mailboxes={} message_links={} referenced_blobs={} orphan_blobs={} pending_deliveries={}",
                report.mailboxes,
                report.message_links,
                report.referenced_blobs,
                report.orphan_blobs,
                report.pending_deliveries
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("check error: {error}");
            ExitCode::FAILURE
        }
    }
}
