//! CLI-граница повторяемой инициализации без секретов в process arguments.

use std::{env, process::ExitCode};

pub(crate) fn initialize() -> ExitCode {
    let Some(data) = data_directory() else {
        return invalid();
    };
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

/// Добавляет только Unicode acceptance-учётную запись отдельным opt-in шагом.
pub(crate) fn initialize_smtp_utf8_test() -> ExitCode {
    let Some(data) = data_directory() else {
        return invalid();
    };
    match litterkeep_server::bootstrap::initialize_smtp_utf8_test(data) {
        Ok(report) => {
            println!(
                "INIT SMTPUTF8 test account ok created={} credentials={}",
                report.created,
                report.credentials_path.display()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("SMTPUTF8 test account init error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Разбирает общую форму каталога, не принимая лишних позиционных аргументов.
fn data_directory() -> Option<String> {
    let mut arguments = env::args().skip(2);
    match (
        arguments.next().as_deref(),
        arguments.next(),
        arguments.next(),
    ) {
        (Some("--data-dir"), Some(path), None) => Some(path),
        _ => None,
    }
}

fn invalid() -> ExitCode {
    super::usage();
    ExitCode::from(2)
}
