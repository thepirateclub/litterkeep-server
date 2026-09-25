//! Ограниченное чтение строк ответов удалённого SMTP-сервера.

use std::io::{self, BufRead, ErrorKind};

/// SMTP command phase, для которой проверяется ожидаемый positive reply.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReplyExpectation {
    Greeting,
    Mail,
    Recipient,
    Data,
    Message,
    Quit,
    Reset,
    Authentication,
}

impl ReplyExpectation {
    /// Проверяет ожидаемую фазу, сохраняя совместимость с новыми 2yz кодами.
    pub(crate) fn accepts(self, code: u16) -> bool {
        match self {
            Self::Greeting => code == 220,
            Self::Data => code == 354,
            Self::Quit => code == 221,
            Self::Authentication => code == 235,
            Self::Mail | Self::Recipient | Self::Message | Self::Reset => {
                code / 100 == 2 && !matches!(code, 220 | 221 | 235)
            }
        }
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
    use super::read_reply_line;
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
}
