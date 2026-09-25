//! Собственный минимальный SMTP client transport для delivery worker-а.
use std::{
    io::{self, BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

#[path = "outbound_data.rs"]
mod data;
#[path = "outbound_ehlo.rs"]
mod ehlo;
#[path = "outbound_mail.rs"]
mod mail;
#[path = "outbound_reply.rs"]
mod reply;

#[cfg(test)]
pub(crate) use data::DataValidationError;
#[cfg(test)]
pub(crate) use data::has_8bit_body;
pub(crate) use data::{validate_data, write_data};
pub(crate) use ehlo::{EhloCapabilities, EhloExtension, SizeAdvertisement, read_ehlo};
pub(crate) use mail::{MailCommand, build_mail_command};
pub(crate) use reply::{ReplyExpectation, read_reply_line};

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
    Unexpected,
}

/// Результат SMTP transaction, который очередь переводит в durable state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryOutcome {
    Delivered,
    Deferred,
    PermanentFailure,
}

/// Одно чистое SMTP-соединение, которым в текущий момент владеет один caller.
///
/// После I/O ошибки объект должен быть уничтожен: peer мог оставить
/// незавершённое состояние MAIL transaction. Известный SMTP отказ может быть
/// очищен только отдельным подтверждённым `RSET`.
pub struct Connection {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    capabilities: EhloCapabilities,
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
            capabilities: EhloCapabilities::default(),
        };
        expect_success(&mut result.reader, ReplyExpectation::Greeting)?;
        result.command("EHLO litterkeep.test\r\n")?;
        result.capabilities = read_ehlo(&mut result.reader)?;
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
        let mail_command = match build_mail_command(sender, message, &self.capabilities) {
            MailCommand::Ready(command) => command,
            MailCommand::PermanentFailure => return Ok(DeliveryOutcome::PermanentFailure),
        };
        self.command(&mail_command)?;
        if let Some(outcome) = expect(&mut self.reader, ReplyExpectation::Mail)? {
            return Ok(outcome);
        }
        self.command(&format!("RCPT TO:<{recipient}>\r\n"))?;
        if let Some(outcome) = expect(&mut self.reader, ReplyExpectation::Recipient)? {
            return Ok(outcome);
        }
        self.command("DATA\r\n")?;
        if let Some(outcome) = expect(&mut self.reader, ReplyExpectation::Data)? {
            return Ok(outcome);
        }
        write_data(&mut self.writer, message)?;
        self.writer.flush()?;
        match expect(&mut self.reader, ReplyExpectation::Message)? {
            Some(outcome) => Ok(outcome),
            None => Ok(DeliveryOutcome::Delivered),
        }
    }

    /// Завершает standalone-соединение. Ошибка QUIT не отменяет уже принятый DATA.
    pub fn quit(&mut self) -> io::Result<()> {
        self.command("QUIT\r\n")?;
        expect_success(&mut self.reader, ReplyExpectation::Quit)
    }

    /// Возвращает peer в известное idle-состояние после SMTP отказа.
    ///
    /// Пул вызывает этот метод только когда полный ответ исходной команды уже
    /// прочитан. Ошибка записи, EOF либо неположительный ответ оставляют
    /// состояние неизвестным, поэтому caller обязан уничтожить connection.
    pub fn reset(&mut self) -> io::Result<()> {
        self.command("RSET\r\n")?;
        expect_success(&mut self.reader, ReplyExpectation::Reset)
    }

    fn command(&mut self, wire: &str) -> io::Result<()> {
        command(&mut self.writer, wire)
    }
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
pub(crate) fn expect<R: BufRead>(
    reader: &mut R,
    expectation: ReplyExpectation,
) -> io::Result<Option<DeliveryOutcome>> {
    let mut line = String::new();
    let mut reply_code = None;
    loop {
        line.clear();
        line = read_reply_line(reader)?;
        let (code, continuation) = parse_reply(&line)?;
        match reply_code {
            Some(initial_code) if initial_code != code => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "SMTP multiline reply changed its status code",
                ));
            }
            Some(_) | None => {}
        }
        if !continuation {
            return match expectation.accepts(code) {
                true => Ok(None),
                false => match classify(code) {
                    ReplyClass::Transient => Ok(Some(DeliveryOutcome::Deferred)),
                    ReplyClass::Permanent => Ok(Some(DeliveryOutcome::PermanentFailure)),
                    ReplyClass::Positive | ReplyClass::Unexpected => Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "SMTP peer returned an unexpected positive reply code",
                    )),
                },
            };
        }
        reply_code = Some(code);
    }
}

fn expect_success<R: BufRead>(reader: &mut R, expectation: ReplyExpectation) -> io::Result<()> {
    match expect(reader, expectation)? {
        None => Ok(()),
        Some(DeliveryOutcome::Deferred) => Err(io::Error::other("SMTP peer deferred command")),
        Some(DeliveryOutcome::PermanentFailure) => {
            Err(io::Error::other("SMTP peer rejected command"))
        }
        Some(DeliveryOutcome::Delivered) => Err(io::Error::other("unexpected SMTP outcome")),
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
        2 => ReplyClass::Positive,
        4 => ReplyClass::Transient,
        5 => ReplyClass::Permanent,
        _ => ReplyClass::Unexpected,
    }
}
