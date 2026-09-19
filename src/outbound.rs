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

/// Отправляет один уже сохранённый RFC822 message на локальный SMTP peer.
///
/// Вызывающий владеет очередью и решает retry по классу ошибки. Этот transport
/// не возвращает соединение в пул: pooling и TLS добавляются отдельным этапом.
pub fn deliver(address: &str, sender: &str, recipient: &str, message: &[u8]) -> io::Result<()> {
    let stream = TcpStream::connect(address)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    expect(&mut reader, ReplyClass::Positive)?;
    let mut writer = stream;
    command(&mut writer, "EHLO litterkeep.test\r\n")?;
    expect(&mut reader, ReplyClass::Positive)?;
    command(&mut writer, &format!("MAIL FROM:<{sender}>\r\n"))?;
    expect(&mut reader, ReplyClass::Positive)?;
    command(&mut writer, &format!("RCPT TO:<{recipient}>\r\n"))?;
    expect(&mut reader, ReplyClass::Positive)?;
    command(&mut writer, "DATA\r\n")?;
    expect(&mut reader, ReplyClass::Positive)?;
    writer.write_all(message)?;
    writer.write_all(b"\r\n.\r\n")?;
    writer.flush()?;
    expect(&mut reader, ReplyClass::Positive)?;
    command(&mut writer, "QUIT\r\n")?;
    expect(&mut reader, ReplyClass::Positive)
}

/// Записывает одну SMTP command без скрытого буферизованного ownership.
fn command(writer: &mut TcpStream, command: &str) -> io::Result<()> {
    writer.write_all(command.as_bytes())?;
    writer.flush()
}

/// Читает complete SMTP reply, включая multiline continuation.
fn expect(reader: &mut BufReader<TcpStream>, expected: ReplyClass) -> io::Result<()> {
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
            return match classify(code) == expected {
                true => Ok(()),
                false => Err(io::Error::other("SMTP peer rejected command")),
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_multiline_marker_without_indexing() {
        assert_eq!(parse_reply("250-feature\r\n").expect("reply"), (250, true));
        assert_eq!(classify(550), ReplyClass::Permanent);
    }
}
