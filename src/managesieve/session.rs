//! ManageSieve состояния до передачи typed запросов storage worker-у.

use super::Command;

#[derive(Default)]
enum Phase {
    #[default]
    Plain,
    Authenticated {
        owner: String,
    },
    AwaitingPlainResponse,
    Closed,
}

/// Ответ отделяет wire format от запросов к account-owned script catalog.
pub enum Reply {
    Text(Vec<u8>),
    StartTls,
    Authenticate { response: String },
    AuthenticateChallenge,
    Unauthenticated(Vec<u8>),
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
    pub fn apply(&mut self, command: Command, tls_active: bool, starttls_available: bool) -> Reply {
        match (&self.phase, command) {
            (Phase::Plain, Command::Capability) => {
                Reply::Text(capability(tls_active, starttls_available, None))
            }
            (Phase::Authenticated { owner }, Command::Capability) => {
                Reply::Text(capability(tls_active, starttls_available, Some(owner)))
            }
            (Phase::Plain, Command::StartTls) if starttls_available && !tls_active => {
                Reply::StartTls
            }
            (Phase::Plain, Command::StartTls) => no("TLS is unavailable or already active"),
            (Phase::Authenticated { .. }, Command::StartTls) => {
                no("TLS is unavailable or already active")
            }
            (Phase::Authenticated { .. }, Command::Unauthenticate) => {
                self.phase = Phase::Plain;
                Reply::Unauthenticated(capability_lines(
                    tls_active,
                    starttls_available,
                    None,
                    b"OK \"unauthenticated\"\r\n",
                ))
            }
            (Phase::Plain, Command::Unauthenticate) => no("invalid command for state"),
            (Phase::Plain, Command::AuthenticatePlain(Some(response))) if tls_active => {
                Reply::Authenticate { response }
            }
            (Phase::Plain, Command::AuthenticatePlain(None)) if tls_active => {
                self.phase = Phase::AwaitingPlainResponse;
                Reply::AuthenticateChallenge
            }
            (Phase::Plain, Command::AuthenticatePlain(_)) => no("encryption required"),
            (Phase::AwaitingPlainResponse, Command::AuthenticateResponse(response)) => {
                self.phase = Phase::Plain;
                match response.as_str() {
                    "*" => no("authentication cancelled"),
                    _ => Reply::Authenticate { response },
                }
            }
            (Phase::AwaitingPlainResponse, Command::Invalid) => {
                self.phase = Phase::Plain;
                no("syntax error")
            }
            (Phase::Plain, Command::Logout) | (Phase::Authenticated { .. }, Command::Logout) => {
                self.phase = Phase::Closed;
                ok("bye")
            }
            (Phase::Authenticated { .. }, Command::HaveSpace { name, size }) => {
                Reply::HaveSpace { name, size }
            }
            (Phase::Authenticated { .. }, Command::PutScript { name, source }) => {
                Reply::Put { name, source }
            }
            (Phase::Authenticated { .. }, Command::ListScripts) => Reply::List,
            (Phase::Authenticated { .. }, Command::SetActive(name)) => Reply::SetActive(name),
            (Phase::Authenticated { .. }, Command::GetScript(name)) => Reply::Get(name),
            (Phase::Authenticated { .. }, Command::DeleteScript(name)) => Reply::Delete(name),
            (Phase::Authenticated { .. }, Command::RenameScript { old, new }) => {
                Reply::Rename { old, new }
            }
            (Phase::Authenticated { .. }, Command::CheckScript(source)) => Reply::Check(source),
            (Phase::Plain, Command::Noop(tag))
            | (Phase::Authenticated { .. }, Command::Noop(tag)) => noop(tag),
            (_, Command::Invalid) => no("syntax error"),
            _ => no("invalid command for state"),
        }
    }

    /// Сообщает reactor-у, что следующая строка принадлежит SASL exchange.
    pub fn awaiting_plain_response(&self) -> bool {
        matches!(self.phase, Phase::AwaitingPlainResponse)
    }

    pub fn authenticated(&mut self, owner: String, accepted: bool, tls_active: bool) -> Reply {
        match (&self.phase, accepted, tls_active) {
            (Phase::Plain, true, true) => {
                self.phase = Phase::Authenticated {
                    owner: owner.clone(),
                };
                Reply::Text(capability_lines(
                    tls_active,
                    false,
                    Some(&owner),
                    b"OK \"authenticated\"\r\n",
                ))
            }
            (Phase::Plain, true, false) => no("encryption required"),
            (Phase::Plain, false, _) => no("authentication failed"),
            _ => no("no authentication pending"),
        }
    }
}

pub fn greeting(tls_active: bool, starttls_available: bool) -> Vec<u8> {
    capability_lines(tls_active, starttls_available, None, b"OK \"ready\"\r\n")
}

fn capability(tls_active: bool, starttls_available: bool, owner: Option<&str>) -> Vec<u8> {
    capability_lines(
        tls_active,
        starttls_available,
        owner,
        b"OK \"capability\"\r\n",
    )
}

/// Единый список extension strings для greeting и явной команды CAPABILITY.
fn capability_lines(
    tls_active: bool,
    starttls_available: bool,
    owner: Option<&str>,
    ending: &[u8],
) -> Vec<u8> {
    let mut output = b"\"IMPLEMENTATION\" \"litterkeep-server\"\r\n".to_vec();
    match (tls_active, starttls_available) {
        (true, _) => output.extend_from_slice(b"\"SASL\" \"PLAIN\"\r\n"),
        (false, true) => output.extend_from_slice(b"\"SASL\" \"\"\r\n"),
        (false, false) => {}
    }
    if starttls_available && !tls_active {
        output.extend_from_slice(b"\"STARTTLS\"\r\n");
    }
    output.extend_from_slice(b"\"UNAUTHENTICATE\"\r\n");
    output.extend_from_slice(
        b"\"SIEVE\" \"fileinto subaddress envelope copy vacation vacation-seconds\"\r\n",
    );
    output.extend_from_slice(b"\"VERSION\" \"1.0\"\r\n");
    if let Some(owner) = owner {
        append_owner(&mut output, owner);
    }
    output.extend_from_slice(ending);
    output
}

/// Добавляет OWNER как quoted-string, экранируя только два ASCII спецсимвола.
fn append_owner(output: &mut Vec<u8>, owner: &str) {
    output.extend_from_slice(b"\"OWNER\" \"");
    for byte in owner.bytes() {
        match byte {
            b'"' | b'\\' => output.push(b'\\'),
            _ => {}
        }
        output.push(byte);
    }
    output.extend_from_slice(b"\"\r\n");
}
fn ok(message: &str) -> Reply {
    Reply::Text(format!("OK \"{message}\"\r\n").into_bytes())
}
fn no(message: &'static str) -> Reply {
    Reply::Text(format!("NO \"{message}\"\r\n").into_bytes())
}

/// Возвращает RFC 5804 NOOP completion и при наличии эхо-tag response code.
fn noop(tag: Option<String>) -> Reply {
    let response = match tag {
        None => b"OK \"noop\"\r\n".to_vec(),
        Some(tag) => {
            let mut response = b"OK (TAG \"".to_vec();
            for character in tag.chars() {
                match character {
                    '"' | '\\' => response.push(b'\\'),
                    _ => {}
                }
                let mut encoded = [0; 4];
                response.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
            }
            response.extend_from_slice(b"\") \"noop\"\r\n");
            response
        }
    };
    Reply::Text(response)
}
