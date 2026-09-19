use std::env;
use std::process::ExitCode;

use litterkeep_server::{PRODUCT, VERSION};

fn usage() {
    println!(
        "{PRODUCT} {VERSION}\n\nUsage:\n  litterkeep-server init [data-dir] [account] [password]\n  litterkeep-server deliver <address> <sender> <recipient> <message-file>\n  litterkeep-server serve [smtp-address] [imap-address] [data-dir]\n  litterkeep-server serve-tls [smtp-address] [imap-address] [data-dir] <certificate.der> <key.der>\n  litterkeep-server serve-starttls [smtp-address] [imap-address] [data-dir] <certificate.der> <key.der>\n  litterkeep-server serve-pop3-starttls [pop3-address] [data-dir] <certificate.der> <key.der>\n  litterkeep-server help\n  litterkeep-server version"
    );
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
        Some("init") => initialize(),
        Some("deliver") => deliver(),
        Some("serve") => serve(),
        Some("serve-tls") => serve_tls(),
        Some("serve-starttls") => serve_starttls(),
        Some("serve-pop3-starttls") => serve_pop3_starttls(),
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
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("delivery error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Создаёт локальный account для проверяемого запуска.
///
/// Значения по умолчанию сохраняют документированный fixture `bob` с паролем
/// `test-password`, но Linux acceptance может создать независимого получателя.
fn initialize() -> ExitCode {
    let mut arguments = env::args().skip(2);
    let data = arguments.next().unwrap_or_else(|| "runtime".into());
    let account = arguments.next().unwrap_or_else(|| "bob".into());
    let password = arguments.next().unwrap_or_else(|| "test-password".into());
    if arguments.next().is_some() {
        usage();
        return ExitCode::from(2);
    }
    let accounts = match litterkeep_server::accounts::Accounts::open(&data) {
        Ok(accounts) => accounts,
        Err(error) => {
            eprintln!("init error: {error}");
            return ExitCode::FAILURE;
        }
    };
    match accounts.create(&account, password.as_bytes()) {
        Ok(()) => {
            println!("created account {account}");
            ExitCode::SUCCESS
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            println!("account {account} already exists");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("init error: {error}");
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

/// Запускает самостоятельный POP3 listener и требует STLS перед PASS.
#[cfg(target_os = "linux")]
fn serve_pop3_starttls() -> ExitCode {
    let mut arguments = env::args().skip(2);
    let pop3 = arguments.next().unwrap_or_else(|| "127.0.0.1:1110".into());
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
    match litterkeep_server::reactor::serve_pop3_starttls(&pop3, data, config) {
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

#[cfg(not(target_os = "linux"))]
fn serve_pop3_starttls() -> ExitCode {
    eprintln!("serve-pop3-starttls requires the Linux poll reactor");
    ExitCode::FAILURE
}
