//! Разбор многострочного EHLO и типизированных SMTP extension keywords.

use std::io::{self, BufRead};

/// Известные capability keywords, влияющие на формат исходящего сообщения.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EhloExtension {
    EightBitMime,
    Chunking,
    BinaryMime,
    SmtpUtf8,
    SizeUnspecified,
    SizeUnlimited,
    SizeMaximum(u64),
    /// Корректный десятичный лимит, превышающий диапазон локального u64.
    SizeMaximumAboveU64,
}

/// Смысл параметра SIZE, объявленного удалённым SMTP-сервером.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum SizeAdvertisement {
    #[default]
    NotAdvertised,
    NoFixedLimit,
    Maximum(u64),
    /// Числовой лимит конечен, но заведомо больше любого локального SIZE value.
    MaximumAboveU64,
}

/// Набор распознанных возможностей текущего SMTP peer.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct EhloCapabilities {
    extensions: Vec<EhloExtension>,
}

/// Итог EHLO: extension mode либо совместимый базовый HELO без capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum EhloNegotiation {
    Extended(EhloCapabilities),
    Basic,
}

impl EhloCapabilities {
    /// Проверяет capability через типизированный keyword, не через сырую строку.
    pub(crate) fn supports(&self, extension: EhloExtension) -> bool {
        match extension {
            EhloExtension::EightBitMime => self
                .extensions
                .iter()
                .any(|item| matches!(item, EhloExtension::EightBitMime)),
            EhloExtension::SmtpUtf8 => self
                .extensions
                .iter()
                .any(|item| matches!(item, EhloExtension::SmtpUtf8)),
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
                EhloExtension::SizeMaximumAboveU64 => Some(SizeAdvertisement::MaximumAboveU64),
                EhloExtension::EightBitMime
                | EhloExtension::Chunking
                | EhloExtension::BinaryMime
                | EhloExtension::SmtpUtf8 => None,
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
#[cfg(test)]
pub(crate) fn read_ehlo(reader: &mut impl BufRead) -> io::Result<EhloCapabilities> {
    read_ehlo_from(reader, None)
}

fn read_ehlo_from(
    reader: &mut impl BufRead,
    initial_line: Option<String>,
) -> io::Result<EhloCapabilities> {
    let mut capabilities = EhloCapabilities::default();
    let mut pending_line = initial_line;
    loop {
        let line = match pending_line.take() {
            Some(initial) => initial,
            None => super::read_reply_line(reader)?,
        };
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

/// Читает EHLO и отличает ответ «команда неизвестна» для RFC fallback.
pub(crate) fn negotiate_ehlo(reader: &mut impl BufRead) -> io::Result<EhloNegotiation> {
    let first_line = super::read_reply_line(reader)?;
    let (code, continuation) = super::parse_reply(&first_line)?;
    match (code, continuation) {
        (500, false) => Ok(EhloNegotiation::Basic),
        (250, _) => read_ehlo_from(reader, Some(first_line)).map(EhloNegotiation::Extended),
        _ => Err(io::Error::other("SMTP EHLO was not accepted")),
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
        "CHUNKING" => Ok(Some(EhloExtension::Chunking)),
        "BINARYMIME" => Ok(Some(EhloExtension::BinaryMime)),
        "SMTPUTF8" => Ok(Some(EhloExtension::SmtpUtf8)),
        "SIZE" => parse_size_advertisement(tokens),
        _ => Ok(None),
    }
}

fn parse_size_advertisement<'a>(
    mut tokens: impl Iterator<Item = &'a str>,
) -> io::Result<Option<EhloExtension>> {
    let extension = match tokens.next() {
        None => EhloExtension::SizeUnspecified,
        Some(value) if value.bytes().all(|byte| byte.is_ascii_digit()) => {
            match value.parse::<u64>() {
                Ok(0) => EhloExtension::SizeUnlimited,
                Ok(maximum) => EhloExtension::SizeMaximum(maximum),
                Err(_) => EhloExtension::SizeMaximumAboveU64,
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
    use super::{EhloExtension, EhloNegotiation, SizeAdvertisement, negotiate_ehlo, read_ehlo};
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
    fn unrecognized_ehlo_is_a_basic_mode_not_an_extension_reply() {
        let mut reply = Cursor::new(b"500 command not recognized\r\n");
        assert!(matches!(
            negotiate_ehlo(&mut reply),
            Ok(EhloNegotiation::Basic)
        ));
    }

    #[test]
    fn rejected_ehlo_policy_does_not_silently_downgrade_to_helo() {
        let mut reply = Cursor::new(b"550 client not allowed\r\n");
        assert!(negotiate_ehlo(&mut reply).is_err());
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

    #[test]
    fn size_capability_accepts_decimal_maximum_above_u64_range() {
        let mut reply = Cursor::new(b"250-peer.test\r\n250 SIZE 18446744073709551616\r\n");
        assert_eq!(
            read_ehlo(&mut reply)
                .expect("large SIZE capability")
                .size_advertisement(),
            SizeAdvertisement::MaximumAboveU64
        );
    }

    #[test]
    fn size_capability_accepts_arbitrarily_many_leading_zeroes() {
        let mut reply = Cursor::new(b"250-peer.test\r\n250 SIZE 00000000000000000000000010\r\n");
        assert_eq!(
            read_ehlo(&mut reply)
                .expect("zero-padded SIZE capability")
                .size_advertisement(),
            SizeAdvertisement::Maximum(10)
        );
    }
}
