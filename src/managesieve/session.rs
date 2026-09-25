//! ManageSieve состояния до передачи typed запросов storage worker-у.

use super::Command;

#[derive(Default)]
enum Phase {
    #[default]
    Plain,
    Authenticated,
    Closed,
}

/// Ответ отделяет wire format от запросов к account-owned script catalog.
pub enum Reply {
    Text(Vec<u8>),
    StartTls,
    Authenticate { response: String },
    HaveSpace { name: String, size: u64 },
    Put { name: String, source: String },
    List,
    SetActive(Option<String>),
    Get(String),
    Delete(String),
    Rename { old: String, new: String },
    Check(String),
}

#[derive(Default)]
pub struct Session {
    phase: Phase,
}

impl Session {
    pub fn apply(&mut self, command: Command, tls_active: bool) -> Reply {
        match (&self.phase, command) {
            (Phase::Plain, Command::Capability) | (Phase::Authenticated, Command::Capability) => {
                Reply::Text(capability(tls_active))
            }
            (Phase::Plain, Command::StartTls) => Reply::StartTls,
            (Phase::Plain, Command::AuthenticatePlain(response)) if tls_active => {
                Reply::Authenticate { response }
            }
            (Phase::Plain, Command::AuthenticatePlain(_)) => no("encryption required"),
            (Phase::Plain, Command::Logout) | (Phase::Authenticated, Command::Logout) => {
                self.phase = Phase::Closed;
                ok("bye")
            }
            (Phase::Authenticated, Command::HaveSpace { name, size }) => {
                Reply::HaveSpace { name, size }
            }
            (Phase::Authenticated, Command::PutScript { name, source }) => {
                Reply::Put { name, source }
            }
            (Phase::Authenticated, Command::ListScripts) => Reply::List,
            (Phase::Authenticated, Command::SetActive(name)) => Reply::SetActive(name),
            (Phase::Authenticated, Command::GetScript(name)) => Reply::Get(name),
            (Phase::Authenticated, Command::DeleteScript(name)) => Reply::Delete(name),
            (Phase::Authenticated, Command::RenameScript { old, new }) => {
                Reply::Rename { old, new }
            }
            (Phase::Authenticated, Command::CheckScript(source)) => Reply::Check(source),
            (Phase::Authenticated, Command::Noop) => ok("noop"),
            (_, Command::Invalid) => no("syntax error"),
            _ => no("invalid command for state"),
        }
    }

    pub fn authenticated(&mut self, accepted: bool) -> Reply {
        match (&self.phase, accepted) {
            (Phase::Plain, true) => {
                self.phase = Phase::Authenticated;
                ok("authenticated")
            }
            (Phase::Plain, false) => no("authentication failed"),
            _ => no("no authentication pending"),
        }
    }
}

fn capability(tls: bool) -> Vec<u8> {
    let mut output = b"\"IMPLEMENTATION\" \"litterkeep-server\"\r\n\"SASL\" \"PLAIN\"\r\n".to_vec();
    if !tls {
        output.extend_from_slice(b"\"STARTTLS\"\r\n");
    }
    output.extend_from_slice(b"\"SIEVE\" \"fileinto subaddress\"\r\nOK \"capability\"\r\n");
    output
}
fn ok(message: &str) -> Reply {
    Reply::Text(format!("OK \"{message}\"\r\n").into_bytes())
}
fn no(message: &'static str) -> Reply {
    Reply::Text(format!("NO \"{message}\"\r\n").into_bytes())
}
