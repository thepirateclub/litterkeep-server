//! Ограниченный пул чистых исходящих SMTP соединений одного маршрута.
//!
//! Пул владеет реальными `TcpStream`, а не только счётчиком. Connection
//! возвращается в idle после positive DATA completion либо подтверждённого RSET.
//! EOF, I/O error или отказ RSET уничтожают канал и освобождают слот, поэтому
//! грязный канал не может быть повторно выдан следующему recipient.
use crate::outbound::{self, Connection, DeliveryOutcome};
use std::{io, net::SocketAddr};

/// Локальный пул одного peer address с жёсткой верхней границей соединений.
pub struct ConnectionPool {
    address: SocketAddr,
    capacity: usize,
    open: usize,
    idle: Vec<Connection>,
}

impl ConnectionPool {
    /// Создаёт пустой pool. Нулевой лимит не маскируется как бесконечный.
    pub fn new(address: impl Into<String>, capacity: usize) -> io::Result<Self> {
        match capacity {
            0 => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "outbound pool capacity must be positive",
            )),
            _ => Ok(Self {
                address: outbound::parse_route(&address.into())?,
                capacity,
                open: 0,
                idle: Vec::new(),
            }),
        }
    }

    /// Выполняет delivery и возвращает channel только в доказанно чистом состоянии.
    pub fn deliver(
        &mut self,
        sender: &str,
        recipient: &str,
        message: &[u8],
    ) -> io::Result<DeliveryOutcome> {
        self.deliver_detailed(sender, recipient, message)
            .map(|attempt| attempt.outcome)
    }

    /// Возвращает SMTP diagnostic до того, как owned socket станет idle.
    pub(crate) fn deliver_detailed(
        &mut self,
        sender: &str,
        recipient: &str,
        message: &[u8],
    ) -> io::Result<outbound::DeliveryAttempt> {
        self.deliver_detailed_with_dsn(sender, recipient, message, None)
    }

    /// Передаёт durable DSN metadata в реальную SMTP transaction.
    pub(crate) fn deliver_detailed_with_dsn(
        &mut self,
        sender: &str,
        recipient: &str,
        message: &[u8],
        metadata: Option<&crate::queue::DsnDeliveryMetadata>,
    ) -> io::Result<outbound::DeliveryAttempt> {
        let mut connection = self.acquire()?;
        let result = connection.deliver_with_dsn(sender, recipient, message, metadata);
        let diagnostic = connection.last_diagnostic().map(|value| {
            value.with_remote_mta(crate::smtp_reply_diagnostic::RemoteMtaIdentity::from_ip(
                self.address.ip(),
            ))
        });
        match result {
            Ok(outcome @ (DeliveryOutcome::Delivered | DeliveryOutcome::Relayed)) => {
                self.idle.push(connection);
                Ok(outbound::DeliveryAttempt {
                    outcome,
                    diagnostic: None,
                })
            }
            Ok(outcome) => {
                match connection.should_close() {
                    true => self.discard(connection),
                    false => match connection.reset() {
                        Ok(()) => self.idle.push(connection),
                        Err(_) => self.discard(connection),
                    },
                }
                Ok(outbound::DeliveryAttempt {
                    outcome,
                    diagnostic,
                })
            }
            Err(error) => {
                self.discard(connection);
                Err(error)
            }
        }
    }

    /// Наблюдаемое число чистых каналов, доступных для следующего job.
    pub fn idle_count(&self) -> usize {
        self.idle.len()
    }

    /// Наблюдаемое число всех живых или idle каналов в лимите маршрута.
    pub fn open_count(&self) -> usize {
        self.open
    }

    /// Завершает каждую idle session командой QUIT и сообщает первую ошибку.
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
            None => match self.open < self.capacity {
                true => self.open_new(),
                false => Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "outbound pool capacity exhausted",
                )),
            },
        }
    }

    fn open_new(&mut self) -> io::Result<Connection> {
        self.open = self
            .open
            .checked_add(1)
            .ok_or_else(|| io::Error::other("outbound pool counter exhausted"))?;
        match Connection::connect_addr(self.address) {
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
