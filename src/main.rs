use std::env;
use std::process::ExitCode;

use litterkeep_server::{PRODUCT, VERSION};

fn usage() {
    println!(
        "{PRODUCT} {VERSION}\n\nUsage:\n  litterkeep-server help\n  litterkeep-server version\n  litterkeep-server serve [smtp-address] [imap-address] [data-dir]"
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
        Some("serve") => serve(),
        Some("serve-smtp") => {
            let address = env::args()
                .nth(2)
                .unwrap_or_else(|| "127.0.0.1:2525".into());
            let data = env::args().nth(3).unwrap_or_else(|| "runtime".into());
            match litterkeep_server::server::serve_smtp(&address, data) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("server error: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("serve-imap") => {
            let address = env::args()
                .nth(2)
                .unwrap_or_else(|| "127.0.0.1:2143".into());
            let data = env::args().nth(3).unwrap_or_else(|| "runtime".into());
            match litterkeep_server::server::serve_imap(&address, data) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("server error: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        Some(command) => {
            eprintln!("unsupported command in this build: {command}\n");
            usage();
            ExitCode::from(2)
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
