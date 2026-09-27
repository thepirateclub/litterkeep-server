//! Проверки SMTP HELP и объявляемого списка реализованных команд.

use super::help;
use crate::reactor::{Kind, Session};

/// HELP перечисляет команды, реально реализованные SMTP wire handler-ом.
#[test]
fn help_lists_supported_auth_tls_and_chunking_commands() {
    let mut session = smtp_session();
    help(&mut session, None).expect("write SMTP HELP response");

    for command in [b"AUTH".as_slice(), b"STARTTLS", b"BDAT"] {
        assert!(
            session
                .output
                .windows(command.len())
                .any(|part| part == command),
            "HELP omits supported command {command:?}"
        );
    }
}

/// Создаёт локальную SMTP-сессию, не открывая внешний listener.
fn smtp_session() -> Session {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test socket");
    let client = std::net::TcpStream::connect(listener.local_addr().expect("local address"))
        .expect("connect test socket");
    let (server, _) = listener.accept().expect("accept test socket");
    drop(client);
    Session::new(Kind::Smtp, server, 1, None, None).expect("create SMTP session")
}
