//! Собственный минимальный SMTP client transport для delivery worker-а.
use std::{
    io::{self, BufReader, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

#[path = "outbound_bdat.rs"]
pub(crate) mod bdat;
#[path = "outbound_connect.rs"]
mod connect;
#[path = "outbound_data.rs"]
mod data;
#[path = "outbound_ehlo.rs"]
mod ehlo;
#[path = "outbound_mail.rs"]
mod mail;
#[path = "outbound_reply.rs"]
mod reply;
#[path = "outbound_timeouts.rs"]
mod timeouts;

#[cfg(target_os = "linux")]
pub(crate) use bdat::validate_bdat_binary;
pub(crate) use bdat::{has_binary_transfer, validate_binary};
#[cfg(test)]
pub(crate) use data::DataValidationError;
#[cfg(test)]
pub(crate) use data::has_8bit_body;
pub(crate) use data::{validate_data, write_data};
pub(crate) use ehlo::negotiate_ehlo;
pub(crate) use ehlo::{EhloCapabilities, EhloExtension, EhloNegotiation, SizeAdvertisement};
#[cfg(test)]
pub(crate) use mail::header_profile::has_utf8_headers;
pub(crate) use mail::{MailCommand, build_mail_command};
pub(crate) use reply::{ReplyExpectation, ReplyResult, expect, expect_success};
pub(crate) use reply::{parse_reply, read_reply_line};
pub(crate) use timeouts::SmtpTimeouts;
#[cfg(test)]
#[path = "outbound_bdat_timeout_tests.rs"]
mod bdat_timeout_tests;
#[cfg(test)]
#[path = "outbound_binarymime_tests.rs"]
mod binarymime_tests;
#[cfg(test)]
#[path = "outbound_quit_timeout_tests.rs"]
mod quit_timeout_tests;
#[cfg(test)]
pub(crate) use ehlo::read_ehlo;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const GENERIC_IO_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy)]
pub(crate) struct TransportLimits {
    connect: Duration,
    timeouts: SmtpTimeouts,
}

impl TransportLimits {
    fn configured() -> io::Result<Self> {
        Ok(Self {
            connect: CONNECT_TIMEOUT,
            timeouts: SmtpTimeouts::from_environment()?,
        })
    }

    #[cfg(test)]
    pub(crate) fn testing(connect: Duration, io: Duration) -> Self {
        Self {
            connect,
            timeouts: SmtpTimeouts::uniform(io),
        }
    }

    #[cfg(test)]
    pub(crate) fn bdat_phases(connect: Duration, command: Duration, termination: Duration) -> Self {
        Self {
            connect,
            timeouts: SmtpTimeouts::bdat_phases(command, termination),
        }
    }
}

/// Категория SMTP-ответа, не зависящая от строковых префиксов.
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
    close_after_response: bool,
    timeouts: SmtpTimeouts,
}

impl Connection {
    /// Открывает TCP и устанавливает расширенную либо базовую SMTP session.
    pub fn connect(address: &str) -> io::Result<Self> {
        Self::connect_addr(parse_route(address)?)
    }

    pub(crate) fn connect_addr(address: SocketAddr) -> io::Result<Self> {
        Self::connect_addr_with_limits(address, TransportLimits::configured()?)
    }

    #[cfg(test)]
    pub(crate) fn connect_with_limits(address: &str, limits: TransportLimits) -> io::Result<Self> {
        Self::connect_addr_with_limits(parse_route(address)?, limits)
    }

    fn connect_addr_with_limits(address: SocketAddr, limits: TransportLimits) -> io::Result<Self> {
        connect::connect_addr_with_limits(address, limits)
    }

    /// Выполняет ровно одну SMTP envelope transaction на уже проверенном канале.
    pub fn deliver(
        &mut self,
        sender: &str,
        recipient: &str,
        message: &[u8],
    ) -> io::Result<DeliveryOutcome> {
        let binary_transfer = has_binary_transfer(message);
        let valid_message = match binary_transfer {
            true => validate_binary(message),
            false => validate_data(message).is_ok(),
        };
        if !valid_message {
            return Ok(DeliveryOutcome::PermanentFailure);
        }
        let envelope_requires_smtp_utf8 =
            crate::smtp::requires_smtp_utf8(sender) || crate::smtp::requires_smtp_utf8(recipient);
        let mail_command = match build_mail_command(
            sender,
            envelope_requires_smtp_utf8,
            message,
            &self.capabilities,
        ) {
            MailCommand::Ready(command) => command,
            MailCommand::PermanentFailure => return Ok(DeliveryOutcome::PermanentFailure),
        };
        self.command(&mail_command)?;
        if let Some(outcome) = self.expect_command(ReplyExpectation::Mail)? {
            return Ok(outcome);
        }
        self.command(&format!("RCPT TO:<{recipient}>\r\n"))?;
        if let Some(outcome) = self.expect_command(ReplyExpectation::Recipient)? {
            return Ok(outcome);
        }
        if binary_transfer {
            self.writer
                .set_write_timeout(Some(self.timeouts.data_block()))?;
            let timeouts = self.timeouts;
            let reply = bdat::send(
                &mut self.reader,
                message,
                |reader, command, payload, last| {
                    let phase = match last {
                        true => ReplyExpectation::BdatLast,
                        false => ReplyExpectation::Bdat,
                    };
                    reader
                        .get_ref()
                        .set_read_timeout(Some(timeouts.response(phase)))?;
                    self.writer.write_all(command)?;
                    self.writer.write_all(payload)?;
                    self.writer.flush()
                },
            )?;
            return Ok(bdat::delivery_outcome(
                reply,
                &mut self.close_after_response,
            ));
        }
        self.command("DATA\r\n")?;
        if let Some(outcome) = self.expect_command(ReplyExpectation::Data)? {
            return Ok(outcome);
        }
        self.writer
            .set_write_timeout(Some(self.timeouts.data_block()))?;
        write_data(&mut self.writer, message)?;
        self.writer.flush()?;
        match self.expect_command(ReplyExpectation::Message)? {
            Some(outcome) => Ok(outcome),
            None => Ok(DeliveryOutcome::Delivered),
        }
    }

    /// Завершает standalone-соединение. Ошибка QUIT не отменяет уже принятый DATA.
    pub fn quit(&mut self) -> io::Result<()> {
        self.command("QUIT\r\n")?;
        self.expect_success(ReplyExpectation::Quit)
    }

    /// Возвращает peer в известное idle-состояние после SMTP отказа.
    ///
    /// Пул вызывает этот метод только когда полный ответ исходной команды уже
    /// прочитан. Ошибка записи, EOF либо неположительный ответ оставляют
    /// состояние неизвестным, поэтому caller обязан уничтожить connection.
    pub fn reset(&mut self) -> io::Result<()> {
        self.command("RSET\r\n")?;
        self.expect_success(ReplyExpectation::Reset)
    }

    fn command(&mut self, wire: &str) -> io::Result<()> {
        self.writer
            .set_write_timeout(Some(self.timeouts.command()))?;
        command(&mut self.writer, wire)
    }

    /// Даёт pool-у знать, что peer потребовал завершить именно этот socket.
    pub(crate) fn should_close(&self) -> bool {
        self.close_after_response
    }

    fn expect_command(
        &mut self,
        expectation: ReplyExpectation,
    ) -> io::Result<Option<DeliveryOutcome>> {
        self.reader
            .get_ref()
            .set_read_timeout(Some(self.timeouts.response(expectation)))?;
        match expect(&mut self.reader, expectation)? {
            ReplyResult::Accepted => Ok(None),
            ReplyResult::Rejected(outcome) => Ok(Some(outcome)),
            ReplyResult::Closing => {
                self.close_after_response = true;
                Ok(Some(DeliveryOutcome::Deferred))
            }
        }
    }

    fn expect_success(&mut self, expectation: ReplyExpectation) -> io::Result<()> {
        match self.expect_command(expectation)? {
            None => Ok(()),
            Some(DeliveryOutcome::Deferred) => Err(io::Error::other("SMTP peer deferred command")),
            Some(DeliveryOutcome::PermanentFailure) => {
                Err(io::Error::other("SMTP peer rejected command"))
            }
            Some(DeliveryOutcome::Delivered) => Err(io::Error::other("unexpected SMTP outcome")),
        }
    }
}

/// Создаёт только числовой TCP route с конечными connect/read/write deadline.
pub(crate) fn connect_bounded(address: &str) -> io::Result<TcpStream> {
    let stream = TcpStream::connect_timeout(&parse_route(address)?, CONNECT_TIMEOUT)?;
    stream.set_read_timeout(Some(GENERIC_IO_TIMEOUT))?;
    stream.set_write_timeout(Some(GENERIC_IO_TIMEOUT))?;
    Ok(stream)
}

pub(crate) fn connect_bounded_with_timeouts(
    address: &str,
    timeouts: SmtpTimeouts,
) -> io::Result<TcpStream> {
    connect_bounded_addr(
        parse_route(address)?,
        TransportLimits {
            connect: CONNECT_TIMEOUT,
            timeouts,
        },
    )
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
    stream.set_read_timeout(Some(limits.timeouts.greeting()))?;
    stream.set_write_timeout(Some(limits.timeouts.command()))?;
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
