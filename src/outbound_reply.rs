//! Ограниченное чтение строк ответов удалённого SMTP-сервера.

use super::DeliveryOutcome;
use std::io::{self, BufRead, ErrorKind};

/// SMTP command phase, для которой проверяется ожидаемый positive reply.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReplyExpectation {
    Greeting,
    Helo,
    Mail,
    Recipient,
    Data,
    Bdat,
    BdatLast,
    Message,
    Quit,
    Reset,
    Authentication,
}

/// Результат SMTP reply сохраняет отдельно смысл 421 о закрытии канала.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReplyResult {
    Accepted,
    Rejected(DeliveryOutcome),
    Closing,
}

/// Класс SMTP-кода отделяет успешный ответ от временного и постоянного отказа.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReplyClass {
    Positive,
    Transient,
    Permanent,
    Unexpected,
}

impl ReplyExpectation {
    /// Проверяет ожидаемую фазу, сохраняя совместимость с новыми 2yz кодами.
    pub(crate) fn accepts(self, code: u16) -> bool {
        match self {
            Self::Greeting => code == 220,
            Self::Helo => code == 250,
            Self::Data => code == 354,
            Self::Bdat | Self::BdatLast => code == 250,
            Self::Quit => code == 221,
            Self::Authentication => code == 235,
            Self::Mail | Self::Recipient | Self::Message | Self::Reset => {
                code / 100 == 2 && !matches!(code, 220 | 221 | 235)
            }
        }
    }
}

/// Читает complete SMTP reply, включая multiline continuation.
pub(crate) fn expect<R: BufRead>(
    reader: &mut R,
    expectation: ReplyExpectation,
) -> io::Result<ReplyResult> {
    let mut reply_code = None;
    loop {
        let line = read_reply_line(reader)?;
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
            if code == 421 {
                return Ok(ReplyResult::Closing);
            }
            return match expectation.accepts(code) {
                true => Ok(ReplyResult::Accepted),
                false => match classify(code) {
                    ReplyClass::Transient => Ok(ReplyResult::Rejected(DeliveryOutcome::Deferred)),
                    ReplyClass::Permanent => {
                        Ok(ReplyResult::Rejected(DeliveryOutcome::PermanentFailure))
                    }
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

/// Превращает ответ фазы в ошибку standalone SMTP операции.
pub(crate) fn expect_success<R: BufRead>(
    reader: &mut R,
    expectation: ReplyExpectation,
) -> io::Result<()> {
    match expect(reader, expectation)? {
        ReplyResult::Accepted => Ok(()),
        ReplyResult::Rejected(DeliveryOutcome::Deferred) => {
            Err(io::Error::other("SMTP peer deferred command"))
        }
        ReplyResult::Rejected(DeliveryOutcome::PermanentFailure) => {
            Err(io::Error::other("SMTP peer rejected command"))
        }
        ReplyResult::Closing => Err(io::Error::new(
            io::ErrorKind::ConnectionAborted,
            "SMTP peer sent 421 and is closing the channel",
        )),
        ReplyResult::Rejected(DeliveryOutcome::Delivered) => {
            Err(io::Error::other("unexpected SMTP outcome"))
        }
    }
}

/// Разбирает числовой SMTP code и continuation marker на protocol boundary.
pub(crate) fn parse_reply(line: &str) -> io::Result<(u16, bool)> {
    let body = line.strip_suffix("\r\n").ok_or_else(|| {
        io::Error::new(ErrorKind::InvalidData, "SMTP reply line must end with CRLF")
    })?;
    let code = body
        .get(..3)
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "short SMTP reply"))?;
    let valid_code = code.bytes().all(|byte| byte.is_ascii_digit());
    if !valid_code {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "SMTP reply code must contain three decimal digits",
        ));
    }
    let number = code
        .parse()
        .map_err(|_| io::Error::new(ErrorKind::InvalidData, "invalid SMTP reply code"))?;
    let suffix = body
        .get(3..)
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "short SMTP reply"))?;
    match suffix {
        "" | " " => Ok((number, false)),
        value if value.starts_with(' ') => Ok((number, false)),
        value if value.starts_with('-') => Ok((number, true)),
        _ => Err(io::Error::new(
            ErrorKind::InvalidData,
            "invalid SMTP reply separator",
        )),
    }
}

fn classify(code: u16) -> ReplyClass {
    match code / 100 {
        2 => ReplyClass::Positive,
        4 => ReplyClass::Transient,
        5 => ReplyClass::Permanent,
        _ => ReplyClass::Unexpected,
    }
}

/// Максимальная длина строки ответа вместе с кодом и CRLF по RFC 5321.
const MAX_REPLY_LINE_OCTETS: usize = 512;

/// Читает одну корректно ограниченную SMTP reply line.
///
/// Буфер накапливает не более нормативного лимита; чрезмерная строка сразу
/// завершает чтение с ошибкой, после чего caller закрывает соединение.
pub(crate) fn read_reply_line(reader: &mut impl BufRead) -> io::Result<String> {
    let mut octets = Vec::with_capacity(MAX_REPLY_LINE_OCTETS);
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Err(io::Error::new(
                ErrorKind::UnexpectedEof,
                "SMTP peer closed before completing a reply line",
            ));
        }
        let newline = available.iter().position(|octet| *octet == b'\n');
        let consumed = match newline {
            Some(position) => position.saturating_add(1),
            None => available.len(),
        };
        if octets.len().saturating_add(consumed) > MAX_REPLY_LINE_OCTETS {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "SMTP reply line exceeds 512 octets",
            ));
        }
        let segment = available.get(..consumed).ok_or_else(|| {
            io::Error::new(ErrorKind::InvalidData, "invalid SMTP reply buffer segment")
        })?;
        octets.extend_from_slice(segment);
        reader.consume(consumed);
        match newline {
            Some(_) => break,
            None => continue,
        }
    }
    if !octets.ends_with(b"\r\n") {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "SMTP reply line must end with CRLF",
        ));
    }
    String::from_utf8(octets)
        .map_err(|_| io::Error::new(ErrorKind::InvalidData, "SMTP reply is not valid UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::{parse_reply, read_reply_line};
    use std::io::{Cursor, ErrorKind};

    #[test]
    fn accepts_reply_line_at_the_512_octet_boundary() {
        let mut wire = b"250 ".to_vec();
        wire.extend(std::iter::repeat_n(b'x', 506));
        wire.extend_from_slice(b"\r\n");
        assert_eq!(wire.len(), 512);
        let mut reader = Cursor::new(wire.clone());
        assert_eq!(read_reply_line(&mut reader).expect("reply line").len(), 512);
        assert_eq!(reader.position(), 512);
    }

    #[test]
    fn rejects_reply_line_one_octet_over_the_512_octet_boundary() {
        let mut wire = b"250 ".to_vec();
        wire.extend(std::iter::repeat_n(b'x', 507));
        wire.extend_from_slice(b"\r\n");
        let mut reader = Cursor::new(wire);
        assert_eq!(
            read_reply_line(&mut reader)
                .expect_err("oversized reply")
                .kind(),
            ErrorKind::InvalidData
        );
    }

    #[test]
    fn rejects_reply_line_without_crlf_terminator() {
        let mut reader = Cursor::new(b"250 peer\n");
        assert_eq!(
            read_reply_line(&mut reader)
                .expect_err("bare LF reply")
                .kind(),
            ErrorKind::InvalidData
        );
    }

    /// RFC 5321 §4.2 требует принимать код без текста и с пустым текстом.
    #[test]
    fn accepts_reply_code_without_text_or_with_only_separator() {
        assert_eq!(parse_reply("250\r\n").expect("bare code"), (250, false));
        assert_eq!(parse_reply("250 \r\n").expect("empty text"), (250, false));
        assert_eq!(parse_reply("250 text\r\n").expect("text"), (250, false));
        assert_eq!(
            parse_reply("250-\r\n").expect("empty continuation"),
            (250, true)
        );
    }

    /// Reply code остаётся строго тремя ASCII цифрами и CRLF-строкой.
    #[test]
    fn rejects_non_numeric_code_and_missing_crlf() {
        assert_eq!(
            parse_reply("+25 text\r\n")
                .expect_err("sign is not a reply digit")
                .kind(),
            ErrorKind::InvalidData
        );
        assert_eq!(
            parse_reply("250 text\n")
                .expect_err("bare LF is not a complete reply line")
                .kind(),
            ErrorKind::InvalidData
        );
    }
}

#[cfg(test)]
#[path = "outbound_bdat_reply_tests.rs"]
mod bdat_tests;
