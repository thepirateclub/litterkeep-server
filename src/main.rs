use std::env;
use std::process::ExitCode;

use litterkeep_server::{PRODUCT, VERSION};

fn usage() {
    println!(
        "{PRODUCT} {VERSION}\n\nUsage:\n  litterkeep-server help\n  litterkeep-server version\n\nThe network service is introduced after the lifecycle, durable-store, and TLS gates."
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
