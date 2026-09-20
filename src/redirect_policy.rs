//! Общая typed policy для Sieve redirect и исходящего SMTP worker-а.

use std::fmt;

const MAX_ADDRESS_BYTES: usize = 254;
const MAX_LOCAL_BYTES: usize = 64;
const MAX_RECEIVED_HOPS: usize = 8;

/// Разрешённый домен фиксированного локального route-профиля.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Domain {
    PeerTest,
}

impl Domain {
    fn parse(value: &str) -> Result<Self, Error> {
        match value.eq_ignore_ascii_case("peer.test") {
            true => Ok(Self::PeerTest),
            false => Err(Error::RouteDenied),
        }
    }

    fn wire(self) -> &'static str {
        match self {
            Self::PeerTest => "peer.test",
        }
    }
}

/// Проверенный адрес сохраняет исходный local-part и typed route domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Recipient<'a> {
    local: &'a str,
    pub domain: Domain,
}

impl<'a> Recipient<'a> {
    /// Разбирает один ограниченный RFC 5321 dot-atom mailbox без display name.
    pub fn parse(address: &'a str) -> Result<Self, Error> {
        let (local, domain) = address.split_once('@').ok_or(Error::Malformed)?;
        match (
            address.len() > MAX_ADDRESS_BYTES,
            domain.contains('@'),
            local_valid(local),
        ) {
            (false, false, true) => Ok(Self {
                local,
                domain: Domain::parse(domain)?,
            }),
            _ => Err(Error::Malformed),
        }
    }

    pub fn canonical(self) -> String {
        format!("{}@{}", self.local, self.domain.wire())
    }
}

/// Причина запрета до открытия transport socket-а.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Malformed,
    RouteDenied,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Malformed => "malformed redirect recipient",
            Self::RouteDenied => "redirect route is not allowed",
        })
    }
}

impl std::error::Error for Error {}

/// Ограничивает повторную пересылку по trace headers без изменения MIME bytes.
pub fn hop_limit_reached(headers: &[(String, String)]) -> bool {
    headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("received"))
        .take(MAX_RECEIVED_HOPS)
        .count()
        >= MAX_RECEIVED_HOPS
}

fn local_valid(local: &str) -> bool {
    match (
        local.is_empty(),
        local.len() > MAX_LOCAL_BYTES,
        local.split('.').any(str::is_empty),
    ) {
        (false, false, false) => local
            .split('.')
            .all(|part| part.chars().all(atom_character)),
        _ => false,
    }
}

fn atom_character(value: char) -> bool {
    matches!(
        value,
        'a'..='z'
            | 'A'..='Z'
            | '0'..='9'
            | '!'
            | '#'
            | '$'
            | '%'
            | '&'
            | '\''
            | '*'
            | '+'
            | '-'
            | '/'
            | '='
            | '?'
            | '^'
            | '_'
            | '`'
            | '{'
            | '|'
            | '}'
            | '~'
    )
}
