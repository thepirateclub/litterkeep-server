//! Bounded pool исходящих SMTP-соединений с неизменяемым TLS context.

use crate::{outbound::DeliveryOutcome, outbound_tls::Connection};
use std::io;

/// Pool key задаётся address, SNI и CA один раз и не смешивается при reuse.
pub struct ConnectionPool {
    address: String,
    server_name: String,
    ca_der: Vec<u8>,
    capacity: usize,
    open: usize,
    idle: Vec<Connection>,
}

impl ConnectionPool {
    pub fn new(
        address: impl Into<String>,
        server_name: impl Into<String>,
        ca_der: Vec<u8>,
        capacity: usize,
    ) -> io::Result<Self> {
        if capacity == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "outbound TLS pool capacity must be positive",
            ));
        }
        let address = address.into();
        crate::outbound::parse_route(&address)?;
        Ok(Self {
            address,
            server_name: server_name.into(),
            ca_der,
            capacity,
            open: 0,
            idle: Vec::new(),
        })
    }

    pub fn deliver(
        &mut self,
        sender: &str,
        recipient: &str,
        message: &[u8],
    ) -> io::Result<DeliveryOutcome> {
        let mut connection = self.acquire()?;
        match connection.deliver(sender, recipient, message) {
            Ok(DeliveryOutcome::Delivered) => {
                self.idle.push(connection);
                Ok(DeliveryOutcome::Delivered)
            }
            Ok(outcome) => {
                match connection.should_close() {
                    true => self.discard(connection),
                    false => match connection.reset() {
                        Ok(()) => self.idle.push(connection),
                        Err(_) => self.discard(connection),
                    },
                }
                Ok(outcome)
            }
            Err(error) => {
                self.discard(connection);
                Err(error)
            }
        }
    }

    #[must_use]
    pub fn idle_count(&self) -> usize {
        self.idle.len()
    }

    /// Завершает каждую idle TLS session командой QUIT и сообщает первую ошибку.
    ///
    /// Вызов принадлежит явному owner shutdown; сетевой I/O не выполняется в Drop.
    pub fn shutdown(&mut self) -> io::Result<()> {
        let mut first_error = None;
        for mut connection in self.idle.drain(..) {
            if let Err(error) = connection.quit() {
                match first_error.take() {
                    None => first_error = Some(error),
                    Some(existing) => first_error = Some(existing),
                }
            }
        }
        self.open = 0;
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn acquire(&mut self) -> io::Result<Connection> {
        match self.idle.pop() {
            Some(connection) => Ok(connection),
            None if self.open < self.capacity => self.open_new(),
            None => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "outbound TLS pool capacity exhausted",
            )),
        }
    }

    fn open_new(&mut self) -> io::Result<Connection> {
        self.open = self
            .open
            .checked_add(1)
            .ok_or_else(|| io::Error::other("outbound TLS pool counter exhausted"))?;
        match Connection::connect(&self.address, &self.server_name, &self.ca_der) {
            Ok(connection) => Ok(connection),
            Err(error) => {
                self.open -= 1;
                Err(error)
            }
        }
    }

    fn discard(&mut self, connection: Connection) {
        drop(connection);
        self.open -= 1;
    }
}
