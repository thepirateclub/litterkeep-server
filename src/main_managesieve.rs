//! CLI boundary для собственного ManageSieve STARTTLS listener-а.

#[cfg(target_os = "linux")]
use crate::usage;
#[cfg(target_os = "linux")]
use std::env;
use std::process::ExitCode;

#[cfg(target_os = "linux")]
pub(crate) fn serve_starttls() -> ExitCode {
    let mut arguments = env::args().skip(2);
    let address = arguments.next().unwrap_or_else(|| "127.0.0.1:4190".into());
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
    match litterkeep_server::reactor::serve_managesieve_starttls(&address, data, config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("server error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Запускает SMTP, IMAP, POP3 и ManageSieve с общим единственным Store owner.
#[cfg(target_os = "linux")]
pub(crate) fn serve_all_starttls() -> ExitCode {
    let mut arguments = env::args().skip(2);
    let smtp = arguments.next().unwrap_or_else(|| "127.0.0.1:1587".into());
    let imap = arguments.next().unwrap_or_else(|| "127.0.0.1:1143".into());
    let pop3 = arguments.next().unwrap_or_else(|| "127.0.0.1:1110".into());
    let managesieve = arguments.next().unwrap_or_else(|| "127.0.0.1:4190".into());
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
    match litterkeep_server::reactor::serve_all_mail_starttls(
        &smtp,
        &imap,
        &pop3,
        &managesieve,
        data,
        config,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("server error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Запускает SMTP, IMAP, POP3 и ManageSieve с TLS до любого greeting-а.
#[cfg(target_os = "linux")]
pub(crate) fn serve_all_implicit_tls() -> ExitCode {
    let mut arguments = env::args().skip(2);
    let smtp = arguments.next().unwrap_or_else(|| "127.0.0.1:1465".into());
    let imap = arguments.next().unwrap_or_else(|| "127.0.0.1:1993".into());
    let pop3 = arguments.next().unwrap_or_else(|| "127.0.0.1:1995".into());
    let managesieve = arguments.next().unwrap_or_else(|| "127.0.0.1:14190".into());
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
    match litterkeep_server::reactor::serve_all_mail_implicit_tls(
        &smtp,
        &imap,
        &pop3,
        &managesieve,
        data,
        config,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("server error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn serve_starttls() -> ExitCode {
    eprintln!("ManageSieve requires the Linux poll reactor");
    ExitCode::FAILURE
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn serve_all_starttls() -> ExitCode {
    eprintln!("ManageSieve requires the Linux poll reactor");
    ExitCode::FAILURE
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn serve_all_implicit_tls() -> ExitCode {
    eprintln!("implicit-TLS mail reactor requires Linux poll support");
    ExitCode::FAILURE
}
