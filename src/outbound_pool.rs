//! Ограниченный пул чистых исходящих SMTP соединений одного маршрута.
//!
//! Пул владеет реальными `TcpStream`, а не только счётчиком. Connection
//! возвращается в idle после positive DATA completion либо подтверждённого RSET.
//! EOF, I/O error или отказ RSET уничтожают канал и освобождают слот, поэтому
//! грязный канал не может быть повторно выдан следующему recipient.
use crate::outbound::{Connection, DeliveryOutcome};
use std::io;

/// Локальный пул одного peer address с жёсткой верхней границей соединений.
pub struct ConnectionPool {
    address: String,
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
                address: address.into(),
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
        let mut connection = self.acquire()?;
        let result = connection.deliver(sender, recipient, message);
        match result {
            Ok(DeliveryOutcome::Delivered) => {
                self.idle.push(connection);
                Ok(DeliveryOutcome::Delivered)
            }
            Ok(outcome) => {
                match connection.reset() {
                    Ok(()) => self.idle.push(connection),
                    Err(_) => self.discard(connection),
                }
                Ok(outcome)
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
        match Connection::connect(&self.address) {
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
