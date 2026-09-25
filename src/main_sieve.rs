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
    for line in diagnostics_report(&diagnostics) {
        println!("{line}");
    }
    ExitCode::SUCCESS
}

/// Преобразует committed диагностики в TSV без потери фактического fallback.
fn diagnostics_report(diagnostics: &[litterkeep_server::store::SieveDiagnostic]) -> Vec<String> {
    let mut lines = vec!["transaction_id\taccount\tscript_version\terror\toutcome".to_owned()];
    for diagnostic in diagnostics {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}",
            diagnostic.transaction_id,
            diagnostic.account,
            diagnostic.script_version,
            diagnostic.error.code(),
            diagnostic.outcome().code()
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::diagnostics_report;
    use litterkeep_server::sieve::ExecutionError;
    use litterkeep_server::store::SieveDiagnostic;

    /// Проверяет именно строку CLI, включая реально применённое implicit keep.
    #[test]
    fn report_names_runtime_error_and_applied_fallback() {
        let diagnostic = SieveDiagnostic {
            transaction_id: 42,
            account: "bob".to_owned(),
            script_version: 3,
            error: ExecutionError::RedirectLimit,
        };
        assert_eq!(
            diagnostics_report(&[diagnostic]),
            [
                "transaction_id\taccount\tscript_version\terror\toutcome",
                "42\tbob\t3\tredirect-limit\timplicit-keep-only"
            ]
        );
    }
}
