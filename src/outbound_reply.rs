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
    expect_with_diagnostic(reader, expectation).map(|(result, _)| result)
}

/// Читает ответ без потери текста, необходимого для RFC 3464 Diagnostic-Code.
pub(crate) fn expect_with_diagnostic<R: BufRead>(
    reader: &mut R,
    expectation: ReplyExpectation,
) -> io::Result<(
    ReplyResult,
    Option<crate::smtp_reply_diagnostic::SmtpReplyDiagnostic>,
)> {
    let mut reply_code = None;
    let mut reply_text = String::new();
    let mut reply_lines = 0_usize;
    loop {
        let line = read_reply_line(reader)?;
        let (code, continuation) = parse_reply(&line)?;
        append_reply_text(&mut reply_text, &line, &mut reply_lines)?;
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
                let diagnostic =
                    crate::smtp_reply_diagnostic::SmtpReplyDiagnostic::new(code, reply_text)?;
                return Ok((ReplyResult::Closing, Some(diagnostic)));
            }
            return match expectation.accepts(code) {
                true => Ok((ReplyResult::Accepted, None)),
                false => match classify(code) {
                    ReplyClass::Transient => {
                        diagnostic_result(DeliveryOutcome::Deferred, code, reply_text)
                    }
                    ReplyClass::Permanent => {
                        diagnostic_result(DeliveryOutcome::PermanentFailure, code, reply_text)
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

/// Ограничивает multiline ответы и сохраняет читаемый единострочный текст.
fn append_reply_text(output: &mut String, line: &str, count: &mut usize) -> io::Result<()> {
    *count = count.saturating_add(1);
    let body = line
        .strip_suffix("\r\n")
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "SMTP reply line lost CRLF"))?;
    let text = body.get(4..).unwrap_or_default();
    let normalized = text
        .chars()
        .map(|value| match value.is_ascii_graphic() || value == ' ' {
            true => value,
            false => '?',
        });
    let fragment = normalized.collect::<String>();
    let required = output
        .len()
        .saturating_add(fragment.len())
        .saturating_add(1);
    match (*count <= 32, required <= 16_384) {
        (true, true) => {
            if !output.is_empty() && !fragment.is_empty() {
                output.push(' ');
            }
            output.push_str(&fragment);
            Ok(())
        }
        _ => Err(io::Error::new(
            ErrorKind::InvalidData,
            "SMTP multiline diagnostic exceeds configured bound",
        )),
    }
}

fn diagnostic_result(
    outcome: DeliveryOutcome,
    code: u16,
    text: String,
) -> io::Result<(
    ReplyResult,
    Option<crate::smtp_reply_diagnostic::SmtpReplyDiagnostic>,
)> {
    let diagnostic = crate::smtp_reply_diagnostic::SmtpReplyDiagnostic::new(code, text)?;
    Ok((ReplyResult::Rejected(outcome), Some(diagnostic)))
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
#[path = "outbound_bdat_reply_tests.rs"]
mod bdat_tests;
#[cfg(test)]
#[path = "outbound_reply_tests.rs"]
mod tests;
