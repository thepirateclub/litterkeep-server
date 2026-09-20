//! CLI composition для reactor-а с постоянным исходящим scheduler-ом.

#[cfg(target_os = "linux")]
use crate::usage;
#[cfg(target_os = "linux")]
use std::env;
use std::process::ExitCode;

/// Запускает четыре входящих listener-а и маршрут к локальному SMTP peer.
#[cfg(target_os = "linux")]
pub(crate) fn serve() -> ExitCode {
    let mut arguments = env::args().skip(2);
    let smtp = arguments.next().unwrap_or_else(|| "127.0.0.1:1587".into());
    let imap = arguments.next().unwrap_or_else(|| "127.0.0.1:1143".into());
    let pop3 = arguments.next().unwrap_or_else(|| "127.0.0.1:1110".into());
    let sieve = arguments.next().unwrap_or_else(|| "127.0.0.1:4190".into());
    let peer = arguments.next().unwrap_or_else(|| "127.0.0.1:2526".into());
    let data = arguments.next().unwrap_or_else(|| "runtime".into());
    let Some(certificate) = arguments.next() else {
        usage();
        return ExitCode::from(2);
    };
    let Some(key) = arguments.next() else {
        usage();
        return ExitCode::from(2);
    };
    if arguments.next().is_some() {
        usage();
        return ExitCode::from(2);
    }
    let material = match litterkeep_server::tls::ServerMaterial::load(certificate, key) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("TLS material error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let config = match material.server_config() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("TLS material error: {error}");
            return ExitCode::FAILURE;
        }
    };
    match litterkeep_server::reactor::serve_all_mail_outbound_starttls(
        &smtp, &imap, &pop3, &sieve, &peer, data, config,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("server error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn serve() -> ExitCode {
    eprintln!("outbound scheduler requires the Linux poll reactor");
    ExitCode::FAILURE
}
