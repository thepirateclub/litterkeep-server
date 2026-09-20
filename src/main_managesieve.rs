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

#[cfg(not(target_os = "linux"))]
pub(crate) fn serve_starttls() -> ExitCode {
    eprintln!("ManageSieve requires the Linux poll reactor");
    ExitCode::FAILURE
}
