//! CLI boundary для POP3 TLS listener-ов без смешивания с SMTP/IMAP командами.

#[cfg(target_os = "linux")]
use crate::usage;
#[cfg(target_os = "linux")]
use std::env;
use std::process::ExitCode;

#[cfg(target_os = "linux")]
pub(crate) fn serve_starttls() -> ExitCode {
    serve(
        "127.0.0.1:1110",
        litterkeep_server::reactor::serve_pop3_starttls,
    )
}

#[cfg(target_os = "linux")]
pub(crate) fn serve_tls() -> ExitCode {
    serve("127.0.0.1:1995", litterkeep_server::reactor::serve_pop3_tls)
}

#[cfg(target_os = "linux")]
fn serve(
    default_address: &str,
    start: fn(&str, String, std::sync::Arc<rustls::ServerConfig>) -> std::io::Result<()>,
) -> ExitCode {
    let mut arguments = env::args().skip(2);
    let address = arguments.next().unwrap_or_else(|| default_address.into());
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
    match start(&address, data, config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("server error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn serve_starttls() -> ExitCode {
    unavailable("serve-pop3-starttls")
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn serve_tls() -> ExitCode {
    unavailable("serve-pop3-tls")
}

#[cfg(not(target_os = "linux"))]
fn unavailable(command: &str) -> ExitCode {
    eprintln!("{command} requires the Linux poll reactor");
    ExitCode::FAILURE
}
