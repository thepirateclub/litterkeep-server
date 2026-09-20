//! CLI-граница повторяемой инициализации без секретов в process arguments.

use std::{env, process::ExitCode};

pub(crate) fn initialize() -> ExitCode {
    let mut arguments = env::args().skip(2);
    let data = match (arguments.next().as_deref(), arguments.next()) {
        (Some("--data-dir"), Some(path)) => path,
        _ => return invalid(),
    };
    if arguments.next().is_some() {
        return invalid();
    }
    match litterkeep_server::bootstrap::initialize(data) {
        Ok(report) => {
            println!(
                "INIT ok created={} existing={} credentials={}",
                report.created_accounts,
                report.existing_accounts,
                report.credentials_path.display()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("init error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn invalid() -> ExitCode {
    super::usage();
    ExitCode::from(2)
}
