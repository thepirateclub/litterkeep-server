//! Типизированный маршрут и его единственный SMTP connection pool.

use crate::{outbound, outbound_pool, outbound_tls_pool};
use std::{io, net::SocketAddr};

/// Transport policy становится enum до запуска worker-а.
pub enum Route {
    Plain {
        address: SocketAddr,
    },
    ImplicitTls {
        address: SocketAddr,
        server_name: String,
        ca_der: Vec<u8>,
    },
}

impl Route {
    pub fn plain(address: &str) -> io::Result<Self> {
        Ok(Self::Plain {
            address: outbound::parse_route(address)?,
        })
    }

    pub fn implicit_tls(
        address: &str,
        server_name: impl Into<String>,
        ca_der: Vec<u8>,
    ) -> io::Result<Self> {
        Ok(Self::ImplicitTls {
            address: outbound::parse_route(address)?,
            server_name: server_name.into(),
            ca_der,
        })
    }
}

/// Enum не позволяет вернуть TLS connection в plaintext pool или наоборот.
pub(crate) enum Pool {
    Plain(outbound_pool::ConnectionPool),
    ImplicitTls(outbound_tls_pool::ConnectionPool),
}

impl Pool {
    pub(crate) fn new(route: Route, capacity: usize) -> io::Result<Self> {
        match route {
            Route::Plain { address } => {
                outbound_pool::ConnectionPool::new(address.to_string(), capacity).map(Self::Plain)
            }
            Route::ImplicitTls {
                address,
                server_name,
                ca_der,
            } => outbound_tls_pool::ConnectionPool::new(
                address.to_string(),
                server_name,
                ca_der,
                capacity,
            )
            .map(Self::ImplicitTls),
        }
    }

    /// Прокидывает durable DSN policy в выбранный pool без смены route identity.
    pub(crate) fn deliver_detailed_with_dsn(
        &mut self,
        sender: &str,
        recipient: &str,
        payload: &[u8],
        metadata: Option<&crate::queue::DsnDeliveryMetadata>,
    ) -> io::Result<outbound::DeliveryAttempt> {
        let recipient = crate::redirect_policy::Recipient::parse(recipient)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        let recipient = recipient.canonical();
        match self {
            Self::Plain(pool) => {
                pool.deliver_detailed_with_dsn(sender, &recipient, payload, metadata)
            }
            Self::ImplicitTls(pool) => {
                pool.deliver_detailed_with_dsn(sender, &recipient, payload, metadata)
            }
        }
    }

    /// Передаёт explicit shutdown обоим transport pool владельцам.
    pub(crate) fn shutdown(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(pool) => pool.shutdown(),
            Self::ImplicitTls(pool) => pool.shutdown(),
        }
    }
}
