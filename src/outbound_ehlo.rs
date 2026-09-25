//! Разбор многострочного EHLO и типизированных SMTP extension keywords.

use std::io::{self, BufRead};

/// Известные capability keywords, влияющие на формат исходящего сообщения.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EhloExtension {
    EightBitMime,
    SizeUnspecified,
    SizeUnlimited,
    SizeMaximum(u64),
}

/// Смысл параметра SIZE, объявленного удалённым SMTP-сервером.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum SizeAdvertisement {
    #[default]
    NotAdvertised,
    NoFixedLimit,
    Maximum(u64),
}

/// Набор распознанных возможностей текущего SMTP peer.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct EhloCapabilities {
    extensions: Vec<EhloExtension>,
}

impl EhloCapabilities {
    /// Проверяет capability через типизированный keyword, не через сырую строку.
    pub(crate) fn supports(&self, extension: EhloExtension) -> bool {
        match extension {
            EhloExtension::EightBitMime => self
                .extensions
                .iter()
                .any(|item| matches!(item, EhloExtension::EightBitMime)),
            _ => self.extensions.contains(&extension),
        }
    }

    /// Возвращает числовое объявление SIZE либо его отсутствие.
    pub(crate) fn size_advertisement(&self) -> SizeAdvertisement {
        self.extensions
            .iter()
            .find_map(|extension| match extension {
                EhloExtension::SizeUnspecified | EhloExtension::SizeUnlimited => {
                    Some(SizeAdvertisement::NoFixedLimit)
                }
                EhloExtension::SizeMaximum(maximum) => Some(SizeAdvertisement::Maximum(*maximum)),
                EhloExtension::EightBitMime => None,
            })
            .unwrap_or_default()
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
        line = super::read_reply_line(reader)?;
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
    let mut tokens = text.split_ascii_whitespace();
    let Some(keyword) = tokens.next() else {
        return Ok(None);
    };
    match keyword.to_ascii_uppercase().as_str() {
        "8BITMIME" => Ok(Some(EhloExtension::EightBitMime)),
        "SIZE" => parse_size_advertisement(tokens),
        _ => Ok(None),
    }
}

fn parse_size_advertisement<'a>(
    mut tokens: impl Iterator<Item = &'a str>,
) -> io::Result<Option<EhloExtension>> {
    let extension = match tokens.next() {
        None => EhloExtension::SizeUnspecified,
        Some(value) if value.len() <= 20 && value.bytes().all(|byte| byte.is_ascii_digit()) => {
            match value.parse::<u64>() {
                Ok(0) => EhloExtension::SizeUnlimited,
                Ok(maximum) => EhloExtension::SizeMaximum(maximum),
                Err(_) => EhloExtension::SizeMaximum(u64::MAX),
            }
        }
        Some(_) => return Ok(None),
    };
    match tokens.next() {
        Some(_) => Ok(None),
        None => Ok(Some(extension)),
    }
}

#[cfg(test)]
mod tests {
    use super::{EhloExtension, SizeAdvertisement, read_ehlo};
    use std::io::Cursor;

    #[test]
    fn multiline_ehlo_recognizes_case_insensitive_8bitmime_keyword() {
        let mut reply =
            Cursor::new(b"250-peer.test\r\n250-PIPELINING\r\n250-8bitmime\r\n250 SIZE 4096\r\n");
        let capabilities = read_ehlo(&mut reply).expect("EHLO capabilities");
        assert!(capabilities.supports(EhloExtension::EightBitMime));
    }

    #[test]
    fn ehlo_without_8bitmime_keeps_other_advertised_capabilities() {
        let mut reply = Cursor::new(b"250-peer.test\r\n250 SIZE 4096\r\n");
        let capabilities = read_ehlo(&mut reply).expect("EHLO capabilities");
        assert!(!capabilities.supports(EhloExtension::EightBitMime));
        assert_eq!(
            capabilities.size_advertisement(),
            SizeAdvertisement::Maximum(4096)
        );
    }

    #[test]
    fn failed_ehlo_status_is_not_treated_as_a_capability_reply() {
        let mut reply = Cursor::new(b"550 unavailable\r\n");
        assert!(read_ehlo(&mut reply).is_err());
    }

    #[test]
    fn size_capability_preserves_optional_and_zero_limits() {
        let mut unspecified = Cursor::new(b"250-peer.test\r\n250 SIZE\r\n");
        assert_eq!(
            read_ehlo(&mut unspecified)
                .expect("SIZE capability")
                .size_advertisement(),
            SizeAdvertisement::NoFixedLimit
        );
        let mut unlimited = Cursor::new(b"250-peer.test\r\n250 SIZE 0\r\n");
        assert_eq!(
            read_ehlo(&mut unlimited)
                .expect("SIZE capability")
                .size_advertisement(),
            SizeAdvertisement::NoFixedLimit
        );
    }

    #[test]
    fn size_capability_parses_the_advertised_maximum() {
        let mut reply = Cursor::new(b"250-peer.test\r\n250 SIZE 52428800\r\n");
        assert_eq!(
            read_ehlo(&mut reply)
                .expect("SIZE capability")
                .size_advertisement(),
            SizeAdvertisement::Maximum(52_428_800)
        );
    }
}
