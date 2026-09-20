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

/// Отправляет один уже сохранённый RFC822 message на локальный SMTP peer.
///
/// Вызывающий владеет очередью и решает retry по классу ошибки. Этот transport
/// не возвращает соединение в пул: pooling и TLS добавляются отдельным этапом.
pub fn deliver(
    address: &str,
    sender: &str,
    recipient: &str,
    message: &[u8],
) -> io::Result<DeliveryOutcome> {
    let stream = TcpStream::connect(address)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    if let Some(outcome) = expect(&mut reader)? {
        return Ok(outcome);
    }
    let mut writer = stream;
    command(&mut writer, "EHLO litterkeep.test\r\n")?;
    if let Some(outcome) = expect(&mut reader)? {
        return Ok(outcome);
    }
    command(&mut writer, &format!("MAIL FROM:<{sender}>\r\n"))?;
    if let Some(outcome) = expect(&mut reader)? {
        return Ok(outcome);
    }
    command(&mut writer, &format!("RCPT TO:<{recipient}>\r\n"))?;
    if let Some(outcome) = expect(&mut reader)? {
        return Ok(outcome);
    }
    command(&mut writer, "DATA\r\n")?;
    if let Some(outcome) = expect(&mut reader)? {
        return Ok(outcome);
    }
    writer.write_all(message)?;
    writer.write_all(b"\r\n.\r\n")?;
    writer.flush()?;
    if let Some(outcome) = expect(&mut reader)? {
        return Ok(outcome);
    }
    command(&mut writer, "QUIT\r\n")?;
    match expect(&mut reader)? {
        Some(outcome) => Ok(outcome),
        None => Ok(DeliveryOutcome::Delivered),
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
