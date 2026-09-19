use std::env;
use std::process::ExitCode;

use litterkeep_server::{PRODUCT, VERSION};

fn usage() {
    println!(
        "{PRODUCT} {VERSION}\n\nUsage:\n  litterkeep-server init [data-dir]\n  litterkeep-server deliver <address> <sender> <recipient> <message-file>\n  litterkeep-server help\n  litterkeep-server version\n  litterkeep-server serve [smtp-address] [imap-address] [data-dir]"
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

/// Создаёт локальный fixture account для проверяемого development запуска.
fn initialize() -> ExitCode {
    let data = env::args().nth(2).unwrap_or_else(|| "runtime".into());
    let accounts = match litterkeep_server::accounts::Accounts::open(&data) {
        Ok(accounts) => accounts,
        Err(error) => {
            eprintln!("init error: {error}");
            return ExitCode::FAILURE;
        }
    };
    match accounts.create("bob", b"test-password") {
        Ok(()) => {
            println!("created test account bob");
            ExitCode::SUCCESS
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            println!("test account bob already exists");
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

#[cfg(not(target_os = "linux"))]
fn serve() -> ExitCode {
    eprintln!("serve requires the Linux poll reactor; this Windows checkout cannot run it");
    ExitCode::FAILURE
}
