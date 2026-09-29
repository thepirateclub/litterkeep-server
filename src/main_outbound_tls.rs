//! CLI-граница разовой исходящей SMTP transaction с обязательным TLS peer.
use std::{env, process::ExitCode};

/// Читает все security-параметры явно, не подставляя незащищённый default CA.
pub(super) fn deliver() -> ExitCode {
    let mut arguments = env::args().skip(2);
    let Some(address) = arguments.next() else {
        super::usage();
        return ExitCode::from(2);
    };
    let Some(server_name) = arguments.next() else {
        super::usage();
        return ExitCode::from(2);
    };
    let Some(ca_path) = arguments.next() else {
        super::usage();
        return ExitCode::from(2);
    };
    let Some(sender) = arguments.next() else {
        super::usage();
        return ExitCode::from(2);
    };
    let Some(recipient) = arguments.next() else {
        super::usage();
        return ExitCode::from(2);
    };
    let Some(message_path) = arguments.next() else {
        super::usage();
        return ExitCode::from(2);
    };
    if arguments.next().is_some() {
        super::usage();
        return ExitCode::from(2);
    }
    let ca_der = match std::fs::read(ca_path) {
        Ok(value) => value,
        Err(error) => return fail("CA read", error),
    };
    let message = match std::fs::read(message_path) {
        Ok(value) => value,
        Err(error) => return fail("message read", error),
    };
    match litterkeep_server::outbound_tls::deliver(
        &address,
        &server_name,
        &ca_der,
        &sender,
        &recipient,
        &message,
    ) {
        Ok(
            litterkeep_server::outbound::DeliveryOutcome::Delivered
            | litterkeep_server::outbound::DeliveryOutcome::Relayed,
        ) => ExitCode::SUCCESS,
        Ok(litterkeep_server::outbound::DeliveryOutcome::Deferred) => {
            eprintln!("TLS delivery deferred by SMTP peer");
            ExitCode::FAILURE
        }
        Ok(litterkeep_server::outbound::DeliveryOutcome::PermanentFailure) => {
            eprintln!("TLS delivery permanently rejected by SMTP peer");
            ExitCode::FAILURE
        }
        Err(error) => fail("TLS delivery", error),
    }
}

fn fail(context: &str, error: std::io::Error) -> ExitCode {
    eprintln!("{context} error: {error}");
    ExitCode::FAILURE
}
