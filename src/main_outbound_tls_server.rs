//! CLI composition постоянного scheduler-а с implicit-TLS peer route.

#[cfg(target_os = "linux")]
use crate::usage;
use std::process::ExitCode;
#[cfg(target_os = "linux")]
use std::{env, fs};

#[cfg(target_os = "linux")]
pub(crate) fn serve() -> ExitCode {
    let mut values = env::args().skip(2);
    let Some(smtp) = values.next() else {
        return invalid();
    };
    let Some(imap) = values.next() else {
        return invalid();
    };
    let Some(pop3) = values.next() else {
        return invalid();
    };
    let Some(sieve) = values.next() else {
        return invalid();
    };
    let Some(peer) = values.next() else {
        return invalid();
    };
    let Some(peer_name) = values.next() else {
        return invalid();
    };
    let Some(peer_ca) = values.next() else {
        return invalid();
    };
    let Some(data) = values.next() else {
        return invalid();
    };
    let Some(certificate) = values.next() else {
        return invalid();
    };
    let Some(key) = values.next() else {
        return invalid();
    };
    if values.next().is_some() {
        return invalid();
    }
    let ca_der = match fs::read(peer_ca) {
        Ok(value) => value,
        Err(error) => return failure("outbound CA", error),
    };
    let material = match litterkeep_server::tls::ServerMaterial::load(certificate, key) {
        Ok(value) => value,
        Err(error) => return failure("TLS material", error),
    };
    let config = match material.server_config() {
        Ok(value) => value,
        Err(error) => return failure("TLS material", error),
    };
    match litterkeep_server::reactor::serve_all_mail_outbound_tls_starttls(
        &smtp, &imap, &pop3, &sieve, &peer, &peer_name, ca_der, data, config,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => failure("server", error),
    }
}

#[cfg(target_os = "linux")]
fn invalid() -> ExitCode {
    usage();
    ExitCode::from(2)
}

#[cfg(target_os = "linux")]
fn failure(context: &str, error: impl std::fmt::Display) -> ExitCode {
    eprintln!("{context} error: {error}");
    ExitCode::FAILURE
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn serve() -> ExitCode {
    eprintln!("outbound TLS scheduler requires the Linux poll reactor");
    ExitCode::FAILURE
}
