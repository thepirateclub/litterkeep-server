//! Offline CLI для чтения долговечной пользовательской диагностики Sieve.

use crate::usage;
use std::env;
use std::process::ExitCode;

/// Печатает committed runtime-ошибки одного account в стабильном TSV-профиле.
pub(crate) fn errors() -> ExitCode {
    let mut arguments = env::args().skip(2);
    let Some(data_dir) = arguments.next() else {
        usage();
        return ExitCode::from(2);
    };
    let Some(account) = arguments.next() else {
        usage();
        return ExitCode::from(2);
    };
    if arguments.next().is_some() {
        usage();
        return ExitCode::from(2);
    }
    if !std::path::Path::new(&data_dir)
        .join("format-version")
        .is_file()
    {
        eprintln!("sieve diagnostic data directory is not initialized");
        return ExitCode::FAILURE;
    }
    let store = match litterkeep_server::store::Store::open(data_dir) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("sieve diagnostic open error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let diagnostics = match store.sieve_diagnostics(&account) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("sieve diagnostic read error: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("transaction_id\taccount\tscript_version\terror");
    for diagnostic in diagnostics {
        println!(
            "{}\t{}\t{}\t{}",
            diagnostic.transaction_id,
            diagnostic.account,
            diagnostic.script_version,
            diagnostic.error.code()
        );
    }
    ExitCode::SUCCESS
}
