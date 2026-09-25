//! Собственный минимальный SMTP client transport для delivery worker-а.
use std::{
    io::{self, BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const IO_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy)]
pub(crate) struct TransportLimits {
    connect: Duration,
    io: Duration,
}

impl TransportLimits {
    #[cfg(test)]
    pub(crate) fn testing(connect: Duration, io: Duration) -> Self {
        Self { connect, io }
    }
}

impl Default for TransportLimits {
    fn default() -> Self {
        Self {
            connect: CONNECT_TIMEOUT,
            io: IO_TIMEOUT,
        }
    }
}

/// Категория SMTP-ответа, не зависящая от строковых префиксов.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReplyClass {
    Positive,
    Transient,
    Permanent,
}

/// Результат SMTP transaction, который очередь переводит в durable state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryOutcome {
    Delivered,
    Deferred,
    PermanentFailure,
}

/// Причина, по которой RFC 5321 DATA нельзя передать без изменения octets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DataValidationError {
    LineTooLong,
    BareCarriageReturn,
    BareLineFeed,
}

/// Одно чистое SMTP-соединение, которым в текущий момент владеет один caller.
///
/// После I/O ошибки объект должен быть уничтожен: peer мог оставить
/// незавершённое состояние MAIL transaction. Известный SMTP отказ может быть
/// очищен только отдельным подтверждённым `RSET`.
pub struct Connection {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl Connection {
    /// Открывает TCP, проверяет greeting и устанавливает SMTP session через EHLO.
    pub fn connect(address: &str) -> io::Result<Self> {
        Self::connect_addr(parse_route(address)?)
    }

    pub(crate) fn connect_addr(address: SocketAddr) -> io::Result<Self> {
        Self::connect_addr_with_limits(address, TransportLimits::default())
    }

    #[cfg(test)]
    pub(crate) fn connect_with_limits(address: &str, limits: TransportLimits) -> io::Result<Self> {
        Self::connect_addr_with_limits(parse_route(address)?, limits)
    }

    fn connect_addr_with_limits(address: SocketAddr, limits: TransportLimits) -> io::Result<Self> {
        let stream = connect_bounded_addr(address, limits)?;
        let mut result = Self {
            reader: BufReader::new(stream.try_clone()?),
            writer: stream,
        };
        result.expect_positive()?;
        result.command("EHLO litterkeep.test\r\n")?;
        result.expect_positive()?;
        Ok(result)
    }

    /// Выполняет ровно одну SMTP envelope transaction на уже проверенном канале.
    pub fn deliver(
        &mut self,
        sender: &str,
        recipient: &str,
        message: &[u8],
    ) -> io::Result<DeliveryOutcome> {
        if validate_data(message).is_err() {
            return Ok(DeliveryOutcome::PermanentFailure);
        }
        self.command(&format!("MAIL FROM:<{sender}>\r\n"))?;
        if let Some(outcome) = expect(&mut self.reader)? {
            return Ok(outcome);
        }
        self.command(&format!("RCPT TO:<{recipient}>\r\n"))?;
        if let Some(outcome) = expect(&mut self.reader)? {
            return Ok(outcome);
        }
        self.command("DATA\r\n")?;
        if let Some(outcome) = expect(&mut self.reader)? {
            return Ok(outcome);
        }
        write_data(&mut self.writer, message)?;
        self.writer.flush()?;
        match expect(&mut self.reader)? {
            Some(outcome) => Ok(outcome),
            None => Ok(DeliveryOutcome::Delivered),
        }
    }

    /// Завершает standalone-соединение. Ошибка QUIT не отменяет уже принятый DATA.
    pub fn quit(&mut self) -> io::Result<()> {
        self.command("QUIT\r\n")?;
        self.expect_positive()
    }

    /// Возвращает peer в известное idle-состояние после SMTP отказа.
    ///
    /// Пул вызывает этот метод только когда полный ответ исходной команды уже
    /// прочитан. Ошибка записи, EOF либо неположительный ответ оставляют
    /// состояние неизвестным, поэтому caller обязан уничтожить connection.
    pub fn reset(&mut self) -> io::Result<()> {
        self.command("RSET\r\n")?;
        self.expect_positive()
    }

    fn command(&mut self, wire: &str) -> io::Result<()> {
        command(&mut self.writer, wire)
    }

    fn expect_positive(&mut self) -> io::Result<()> {
        match expect(&mut self.reader)? {
            None => Ok(()),
            Some(_) => Err(io::Error::other("SMTP peer rejected connection")),
        }
    }
}

/// Проверяет CRLF framing и RFC 5321 предел строки до начала envelope.
pub(crate) fn validate_data(message: &[u8]) -> Result<(), DataValidationError> {
    let mut line_length = 0_usize;
    let mut pending_cr = false;
    for byte in message.iter().copied() {
        match (pending_cr, byte) {
            (true, b'\n') => {
                line_length = line_length.saturating_add(1);
                if line_length > 1000 {
                    return Err(DataValidationError::LineTooLong);
                }
                line_length = 0;
                pending_cr = false;
            }
            (true, _) => return Err(DataValidationError::BareCarriageReturn),
            (false, b'\r') => {
                line_length = line_length.saturating_add(1);
                pending_cr = true;
            }
            (false, b'\n') => return Err(DataValidationError::BareLineFeed),
            (false, _) => line_length = line_length.saturating_add(1),
        }
        if line_length > 1000 {
            return Err(DataValidationError::LineTooLong);
        }
    }
    if pending_cr {
        return Err(DataValidationError::BareCarriageReturn);
    }
    match line_length.saturating_add(2) > 1000 {
        true => Err(DataValidationError::LineTooLong),
        false => Ok(()),
    }
}

/// Передаёт DATA с ровно одним завершающим CRLF перед SMTP terminator.
pub(crate) fn write_data(writer: &mut impl Write, message: &[u8]) -> io::Result<()> {
    let mut stuffed = Vec::with_capacity(message.len().saturating_add(16));
    let mut line_start = true;
    let mut previous_was_cr = false;
    for byte in message.iter().copied() {
        if line_start && byte == b'.' {
            stuffed.push(b'.');
        }
        stuffed.push(byte);
        line_start = previous_was_cr && byte == b'\n';
        previous_was_cr = byte == b'\r';
    }
    if !stuffed.ends_with(b"\r\n") {
        stuffed.extend_from_slice(b"\r\n");
    }
    writer.write_all(&stuffed)?;
    writer.write_all(b".\r\n")
}

/// Создаёт только числовой TCP route с конечными connect/read/write deadline.
pub(crate) fn connect_bounded(address: &str) -> io::Result<TcpStream> {
    connect_bounded_addr(parse_route(address)?, TransportLimits::default())
}

pub(crate) fn parse_route(address: &str) -> io::Result<SocketAddr> {
    address.parse::<SocketAddr>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "outbound route must be an explicit socket address",
        )
    })
}

fn connect_bounded_addr(address: SocketAddr, limits: TransportLimits) -> io::Result<TcpStream> {
    let stream = TcpStream::connect_timeout(&address, limits.connect)?;
    stream.set_read_timeout(Some(limits.io))?;
    stream.set_write_timeout(Some(limits.io))?;
    Ok(stream)
}

/// Отправляет один RFC822 message без повторного использования соединения.
///
/// CLI использует этот путь для разовой диагностики. Очередь доставки использует
/// `outbound_pool::ConnectionPool`, который возвращает только чистый channel.
pub fn deliver(
    address: &str,
    sender: &str,
    recipient: &str,
    message: &[u8],
) -> io::Result<DeliveryOutcome> {
    let mut connection = Connection::connect(address)?;
    let outcome = connection.deliver(sender, recipient, message)?;
    match outcome {
        DeliveryOutcome::Delivered => {
            let _ = connection.quit();
            Ok(DeliveryOutcome::Delivered)
        }
        DeliveryOutcome::Deferred | DeliveryOutcome::PermanentFailure => Ok(outcome),
    }
}

/// Записывает одну SMTP command без скрытого буферизованного ownership.
fn command(writer: &mut TcpStream, command: &str) -> io::Result<()> {
    writer.write_all(command.as_bytes())?;
    writer.flush()
}

/// Читает complete SMTP reply, включая multiline continuation.
pub(crate) fn expect<R: BufRead>(reader: &mut R) -> io::Result<Option<DeliveryOutcome>> {
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "SMTP peer closed",
            ));
        }
        let (code, continuation) = parse_reply(&line)?;
        if !continuation {
            return match classify(code) {
                ReplyClass::Positive => Ok(None),
                ReplyClass::Transient => Ok(Some(DeliveryOutcome::Deferred)),
                ReplyClass::Permanent => Ok(Some(DeliveryOutcome::PermanentFailure)),
            };
        }
    }
}

/// Разбирает числовой SMTP code и continuation marker на protocol boundary.
pub(crate) fn parse_reply(line: &str) -> io::Result<(u16, bool)> {
    let code = line
        .get(..3)
        .ok_or_else(|| io::Error::other("short SMTP reply"))?;
    let marker = line
        .as_bytes()
        .get(3)
        .copied()
        .ok_or_else(|| io::Error::other("short SMTP reply"))?;
    let number = code
        .parse()
        .map_err(|_| io::Error::other("invalid SMTP reply"))?;
    match marker {
        b' ' => Ok((number, false)),
        b'-' => Ok((number, true)),
        _ => Err(io::Error::other("invalid SMTP separator")),
    }
}

pub(crate) fn classify(code: u16) -> ReplyClass {
    match code / 100 {
        2 | 3 => ReplyClass::Positive,
        4 => ReplyClass::Transient,
        _ => ReplyClass::Permanent,
    }
}
