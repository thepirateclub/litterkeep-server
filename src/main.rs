use std::env;
use std::process::ExitCode;

use litterkeep_server::{PRODUCT, VERSION};
#[path = "main_bootstrap.rs"]
mod bootstrap_cli;
#[path = "main_maintenance.rs"]
mod maintenance_cli;
#[path = "main_managesieve.rs"]
mod managesieve_cli;
#[path = "main_outbound_server.rs"]
mod outbound_server_cli;
#[path = "main_outbound_tls.rs"]
mod outbound_tls_cli;
#[path = "main_outbound_tls_server.rs"]
mod outbound_tls_server_cli;
#[path = "main_pop3.rs"]
mod pop3_cli;
#[path = "main_queue.rs"]
mod queue_cli;
#[path = "main_runtime.rs"]
mod runtime_cli;
#[path = "main_seed.rs"]
mod seed_cli;
#[path = "main_sieve.rs"]
mod sieve_cli;
#[path = "main_usage.rs"]
mod usage_cli;

pub(crate) fn usage() {
    usage_cli::print();
}

fn main() -> ExitCode {
    match env::args().nth(1).as_deref() {
        None | Some("help") | Some("--help") | Some("-h") => {
            usage();
            ExitCode::SUCCESS
        }
        Some("version") | Some("--version") | Some("-V") => {
            println!("{PRODUCT} {VERSION}");
            ExitCode::SUCCESS
        }
        Some("init") => bootstrap_cli::initialize(),
        Some("init-smtputf8-test") => bootstrap_cli::initialize_smtp_utf8_test(),
        Some("check") => maintenance_cli::check(),
        Some("backup") => maintenance_cli::backup(),
        Some("seed") => seed_cli::execute(),
        Some("deliver") => deliver(),
        Some("deliver-tls") => outbound_tls_cli::deliver(),
        Some("queue-drain") => queue_cli::drain(),
        Some("sieve-errors") => sieve_cli::errors(),
        Some("runtime") => runtime_cli::run(),
        Some("serve") => serve(),
        Some("serve-tls") => serve_tls(),
        Some("serve-starttls") => serve_starttls(),
        Some("serve-pop3-starttls") => pop3_cli::serve_starttls(),
        Some("serve-pop3-tls") => pop3_cli::serve_tls(),
        Some("serve-all-starttls") => pop3_cli::serve_all_starttls(),
        Some("serve-managesieve-starttls") => managesieve_cli::serve_starttls(),
        Some("serve-all-mail-starttls") => managesieve_cli::serve_all_starttls(),
        Some("serve-all-mail-tls") => managesieve_cli::serve_all_implicit_tls(),
        Some("serve-all-mail-outbound-starttls") => outbound_server_cli::serve(),
        Some("serve-all-mail-outbound-tls-starttls") => outbound_tls_server_cli::serve(),
        Some(command) => {
            eprintln!("unsupported command in this build: {command}\n");
            usage();
            ExitCode::from(2)
        }
    }
}

/// Отправляет один долговечно подготовленный RFC822 файл через собственный transport.
fn deliver() -> ExitCode {
    let Some(address) = env::args().nth(2) else {
        usage();
        return ExitCode::from(2);
    };
    let Some(sender) = env::args().nth(3) else {
        usage();
        return ExitCode::from(2);
    };
    let Some(recipient) = env::args().nth(4) else {
        usage();
        return ExitCode::from(2);
    };
    let Some(path) = env::args().nth(5) else {
        usage();
        return ExitCode::from(2);
    };
    let message = match std::fs::read(path) {
        Ok(message) => message,
        Err(error) => {
            eprintln!("message read error: {error}");
            return ExitCode::FAILURE;
        }
    };
    match litterkeep_server::outbound::deliver(&address, &sender, &recipient, &message) {
        Ok(litterkeep_server::outbound::DeliveryOutcome::Delivered) => ExitCode::SUCCESS,
        Ok(litterkeep_server::outbound::DeliveryOutcome::Deferred) => {
            eprintln!("delivery deferred by SMTP peer");
            ExitCode::FAILURE
        }
        Ok(litterkeep_server::outbound::DeliveryOutcome::PermanentFailure) => {
            eprintln!("delivery permanently rejected by SMTP peer");
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("delivery error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(target_os = "linux")]
fn serve() -> ExitCode {
    let smtp = env::args()
        .nth(2)
        .unwrap_or_else(|| "127.0.0.1:2525".into());
    let imap = env::args()
        .nth(3)
        .unwrap_or_else(|| "127.0.0.1:2143".into());
    let data = env::args().nth(4).unwrap_or_else(|| "runtime".into());
    match litterkeep_server::reactor::serve(&smtp, &imap, data) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("server error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Запускает только implicit-TLS listener-ы для клиентского SMTP и IMAP.
///
/// По умолчанию используются документированные 1465 и 1993. В отличие от
/// peer-службы `serve`, этот режим не разрешает отправлять greeting или пароль
/// до завершения handshake в reactor.
#[cfg(target_os = "linux")]
fn serve_tls() -> ExitCode {
    let mut arguments = env::args().skip(2);
    let smtp = arguments.next().unwrap_or_else(|| "127.0.0.1:1465".into());
    let imap = arguments.next().unwrap_or_else(|| "127.0.0.1:1993".into());
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
        Ok(material) => material,
        Err(error) => {
            eprintln!("TLS material error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let config = match material.server_config() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("TLS material error: {error}");
            return ExitCode::FAILURE;
        }
    };
    match litterkeep_server::reactor::serve_tls(&smtp, &imap, data, config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("server error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn serve() -> ExitCode {
    eprintln!("serve requires the Linux poll reactor; this Windows checkout cannot run it");
    ExitCode::FAILURE
}

/// Запускает SMTP submission с plaintext greeting и обязательным upgrade.
#[cfg(target_os = "linux")]
fn serve_starttls() -> ExitCode {
    let mut arguments = env::args().skip(2);
    let smtp = arguments.next().unwrap_or_else(|| "127.0.0.1:1587".into());
    let imap = arguments.next().unwrap_or_else(|| "127.0.0.1:1143".into());
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
        Ok(material) => material,
        Err(error) => {
            eprintln!("TLS material error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let config = match material.server_config() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("TLS material error: {error}");
            return ExitCode::FAILURE;
        }
    };
    match litterkeep_server::reactor::serve_starttls(&smtp, &imap, data, config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("server error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn serve_tls() -> ExitCode {
    eprintln!("serve-tls requires the Linux poll reactor; this Windows checkout cannot run it");
    ExitCode::FAILURE
}

#[cfg(not(target_os = "linux"))]
fn serve_starttls() -> ExitCode {
    eprintln!("serve-starttls requires the Linux poll reactor");
    ExitCode::FAILURE
}
