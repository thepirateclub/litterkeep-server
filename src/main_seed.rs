//! CLI seed с секретом из локального файла, а не из списка процессов.

use litterkeep_server::seed::{SeedConfig, run};
use std::{env, fs, process::ExitCode};

pub(crate) fn execute() -> ExitCode {
    let mut arguments = env::args().skip(2);
    let Some(smtp_address) = arguments.next() else {
        return invalid();
    };
    let Some(imap_address) = arguments.next() else {
        return invalid();
    };
    let Some(server_name) = arguments.next() else {
        return invalid();
    };
    let Some(ca_path) = arguments.next() else {
        return invalid();
    };
    let Some(account) = arguments.next() else {
        return invalid();
    };
    let Some(password_path) = arguments.next() else {
        return invalid();
    };
    let Some(bob_password_path) = arguments.next() else {
        return invalid();
    };
    let Some(qa_password_path) = arguments.next() else {
        return invalid();
    };
    if arguments.next().is_some() {
        return invalid();
    }
    let ca_der = match fs::read(ca_path) {
        Ok(value) => value,
        Err(error) => return failed("CA read", error),
    };
    let password = match fs::read(password_path) {
        Ok(value) => trim_line_end(value),
        Err(error) => return failed("password read", error),
    };
    let bob_password = match fs::read(bob_password_path) {
        Ok(value) => trim_line_end(value),
        Err(error) => return failed("Bob password read", error),
    };
    let qa_password = match fs::read(qa_password_path) {
        Ok(value) => trim_line_end(value),
        Err(error) => return failed("QA password read", error),
    };
    let config = SeedConfig {
        smtp_address,
        imap_address,
        server_name,
        ca_der,
        account,
        password,
        bob_password,
        qa_password,
    };
    match run(&config) {
        Ok(report) => {
            println!(
                "SEED ok created={} existing={}",
                report.created, report.existing
            );
            ExitCode::SUCCESS
        }
        Err(error) => failed("seed", error),
    }
}

fn trim_line_end(mut value: Vec<u8>) -> Vec<u8> {
    while matches!(value.last(), Some(b'\r' | b'\n')) {
        value.pop();
    }
    value
}

fn invalid() -> ExitCode {
    super::usage();
    ExitCode::from(2)
}

fn failed(context: &str, error: std::io::Error) -> ExitCode {
    eprintln!("{context} error: {error}");
    ExitCode::FAILURE
}
