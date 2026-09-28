//! Общая typed policy для Sieve redirect и исходящего SMTP worker-а.

use std::fmt;

const MAX_ADDRESS_BYTES: usize = 254;
const MAX_RECEIVED_HOPS: usize = 8;

/// Разрешённый домен фиксированного локального route-профиля.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Domain {
    PeerTest,
    External,
}

impl Domain {
    fn classify(value: &str) -> Self {
        match value.eq_ignore_ascii_case("peer.test") {
            true => Self::PeerTest,
            false => Self::External,
        }
    }
}

/// Проверенный и канонизированный внешний адрес для настроенного relay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recipient {
    canonical_address: String,
    pub domain: Domain,
}

impl Recipient {
    /// Разбирает полный RFC SMTP mailbox и запрещает возврат на локальную цель.
    pub fn parse(address: &str) -> Result<Self, Error> {
        if address.len() > MAX_ADDRESS_BYTES {
            return Err(Error::Malformed);
        }
        let (local, domain) = crate::smtp::canonical_parts(address).ok_or(Error::Malformed)?;
        if crate::local_route::is_server_owned_domain(address) {
            return Err(Error::RouteDenied);
        }
        let domain_kind = Domain::classify(&domain);
        Ok(Self {
            canonical_address: format!("{local}@{domain}"),
            domain: domain_kind,
        })
    }

    pub fn canonical(self) -> String {
        self.canonical_address
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
