//! ManageSieve состояния до передачи typed запросов storage worker-у.

#[path = "session_sasl.rs"]
mod session_sasl;

#[derive(Default)]
pub(super) enum Phase {
    #[default]
    Plain,
    Authenticated {
        owner: String,
    },
    AwaitingPlainResponse,
    AwaitingScramFirstResponse,
    AwaitingScramVerifier,
    AwaitingScramFinal,
    Closed,
}

/// Ответ отделяет wire format от запросов к account-owned script catalog.
pub enum Reply {
    Text(Vec<u8>),
    StartTls,
    Authenticate { response: String },
    AuthenticateChallenge,
    ScramClientFirst { response: String },
    ScramChallenge { response: String },
    ScramAccepted { owner: String, response: Vec<u8> },
    ScramRejected { response: String },
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
    pub(super) phase: Phase,
    pub(super) scram_exchange: Option<ScramExchange>,
}

/// Временный transcript и verifier одного ManageSieve SASL exchange.
pub(super) struct ScramExchange {
    pub(super) owner: String,
    pub(super) client_first: crate::sasl::scram_client_first::ClientFirst,
    pub(super) server_first: Option<crate::sasl::scram_server_first::ServerFirst>,
    pub(super) credential: Option<crate::sasl::scram_sha1_record::ScramSha1Record>,
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
        (true, _) => output.extend_from_slice(b"\"SASL\" \"PLAIN SCRAM-SHA-1\"\r\n"),
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
