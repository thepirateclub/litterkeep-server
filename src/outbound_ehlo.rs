//! Разбор многострочного EHLO и типизированных SMTP extension keywords.

use std::io::{self, BufRead};

/// Известные capability keywords, влияющие на формат исходящего сообщения.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EhloExtension {
    EightBitMime,
}

/// Набор распознанных возможностей текущего SMTP peer.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct EhloCapabilities {
    extensions: Vec<EhloExtension>,
}

impl EhloCapabilities {
    /// Проверяет capability через типизированный keyword, не через сырую строку.
    pub(crate) fn supports(&self, extension: EhloExtension) -> bool {
        self.extensions.contains(&extension)
    }

    fn add(&mut self, extension: EhloExtension) {
        if !self.supports(extension) {
            self.extensions.push(extension);
        }
    }
}

/// Читает все continuation строки успешного EHLO ответа и сохраняет keywords.
pub(crate) fn read_ehlo(reader: &mut impl BufRead) -> io::Result<EhloCapabilities> {
    let mut capabilities = EhloCapabilities::default();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "SMTP peer closed during EHLO response",
            ));
        }
        let (code, continuation) = super::parse_reply(&line)?;
        if code != 250 {
            return Err(io::Error::other("SMTP EHLO was not accepted"));
        }
        if let Some(extension) = parse_extension(&line)? {
            capabilities.add(extension);
        }
        if !continuation {
            return Ok(capabilities);
        }
    }
}

fn parse_extension(line: &str) -> io::Result<Option<EhloExtension>> {
    let text = line
        .get(4..)
        .ok_or_else(|| io::Error::other("short SMTP EHLO capability"))?;
    let Some(keyword) = text.split_ascii_whitespace().next() else {
        return Ok(None);
    };
    match keyword.to_ascii_uppercase().as_str() {
        "8BITMIME" => Ok(Some(EhloExtension::EightBitMime)),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::{EhloCapabilities, EhloExtension, read_ehlo};
    use std::io::Cursor;

    #[test]
    fn multiline_ehlo_recognizes_case_insensitive_8bitmime_keyword() {
        let mut reply =
            Cursor::new(b"250-peer.test\r\n250-PIPELINING\r\n250-8bitmime\r\n250 SIZE 4096\r\n");
        let capabilities = read_ehlo(&mut reply).expect("EHLO capabilities");
        assert!(capabilities.supports(EhloExtension::EightBitMime));
    }

    #[test]
    fn ehlo_without_8bitmime_keeps_the_ascii_only_capability_set() {
        let mut reply = Cursor::new(b"250-peer.test\r\n250 SIZE 4096\r\n");
        assert_eq!(
            read_ehlo(&mut reply).expect("EHLO capabilities"),
            EhloCapabilities::default()
        );
    }

    #[test]
    fn failed_ehlo_status_is_not_treated_as_a_capability_reply() {
        let mut reply = Cursor::new(b"550 unavailable\r\n");
        assert!(read_ehlo(&mut reply).is_err());
    }
}
