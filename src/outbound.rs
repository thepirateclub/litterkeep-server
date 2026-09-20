//! Собственный минимальный SMTP client transport для delivery worker-а.
use std::{
    io::{self, BufRead, BufReader, Write},
    net::TcpStream,
    time::Duration,
};

/// Категория SMTP-ответа, не зависящая от строковых префиксов.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReplyClass {
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

/// Одно чистое SMTP-соединение, которым в текущий момент владеет один caller.
///
/// После ошибки или неуспешного SMTP transaction объект должен быть уничтожен:
/// peer мог оставить незавершённое состояние MAIL transaction. Возврат в pool
/// разрешён только после положительного финального ответа DATA.
pub struct Connection {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl Connection {
    /// Открывает TCP, проверяет greeting и устанавливает SMTP session через EHLO.
    pub fn connect(address: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(address)?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
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
        self.writer.write_all(message)?;
        self.writer.write_all(b"\r\n.\r\n")?;
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
fn expect(reader: &mut BufReader<TcpStream>) -> io::Result<Option<DeliveryOutcome>> {
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
fn parse_reply(line: &str) -> io::Result<(u16, bool)> {
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

fn classify(code: u16) -> ReplyClass {
    match code / 100 {
        2 | 3 => ReplyClass::Positive,
        4 => ReplyClass::Transient,
        _ => ReplyClass::Permanent,
    }
}
