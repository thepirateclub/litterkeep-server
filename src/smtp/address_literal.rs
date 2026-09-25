//! Грамматика IPv4 и зарегистрированных IPv6 SMTP address-literals.

use std::fmt::Write as FmtWrite;
use std::net::Ipv6Addr;

/// Итог проверки доменной части как RFC address-literal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LiteralForm {
    Valid,
    Invalid,
}

/// Стандартизованные метки address-literal, опубликованные реестром IANA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AddressLiteralTag {
    Ipv6,
}

impl AddressLiteralTag {
    /// Преобразует wire tag в enum на границе SMTP-грамматики.
    fn parse(tag: &str) -> Option<Self> {
        match tag.to_ascii_lowercase().as_str() {
            "ipv6" => Some(Self::Ipv6),
            _ => None,
        }
    }
}

/// Проверяет скобочную IPv4 или зарегистрированную IPv6 форму.
pub(super) fn parse_domain(domain: &str) -> LiteralForm {
    let Some(literal) = domain
        .strip_prefix('[')
        .and_then(|content| content.strip_suffix(']'))
    else {
        return LiteralForm::Invalid;
    };
    match literal.split_once(':') {
        Some((tag, address)) => match AddressLiteralTag::parse(tag) {
            Some(AddressLiteralTag::Ipv6) if valid_ipv6_address(address) => LiteralForm::Valid,
            _ => LiteralForm::Invalid,
        },
        None => match valid_ipv4_literal(literal) {
            true => LiteralForm::Valid,
            false => LiteralForm::Invalid,
        },
    }
}

/// Проверяет IPv6 через стандартный parser, нормализуя разрешённые RFC Snum.
fn valid_ipv6_address(address: &str) -> bool {
    match address.rsplit_once(':') {
        Some((prefix, suffix)) if suffix.contains('.') => {
            let Some(ipv4) = normalize_ipv4_address(suffix) else {
                return false;
            };
            let mut normalized = String::with_capacity(address.len());
            normalized.push_str(prefix);
            normalized.push(':');
            normalized.push_str(&ipv4);
            normalized.parse::<Ipv6Addr>().is_ok()
        }
        _ => address.parse::<Ipv6Addr>().is_ok(),
    }
}

/// Проверяет IPv4 без выделения памяти для четырёх входных октетов.
fn valid_ipv4_literal(address: &str) -> bool {
    let mut octets = address.split('.');
    match (
        octets.next(),
        octets.next(),
        octets.next(),
        octets.next(),
        octets.next(),
    ) {
        (Some(first), Some(second), Some(third), Some(fourth), None) => {
            [first, second, third, fourth]
                .iter()
                .all(|octet| valid_ipv4_octet(octet))
        }
        _ => false,
    }
}

/// Проверяет диапазон одного Snum до канонизации или принятия mailbox.
fn valid_ipv4_octet(octet: &str) -> bool {
    match decimal_ipv4_octet(octet) {
        Some(value) => value <= 255,
        None => false,
    }
}

/// Проверяет четыре IPv4 октета и приводит ведущие нули к десятичной форме.
fn normalize_ipv4_address(address: &str) -> Option<String> {
    let mut octets = address.split('.');
    let segments = match (
        octets.next(),
        octets.next(),
        octets.next(),
        octets.next(),
        octets.next(),
    ) {
        (Some(first), Some(second), Some(third), Some(fourth), None) => {
            [first, second, third, fourth]
        }
        _ => return None,
    };
    let mut normalized = String::with_capacity(address.len());
    for segment in segments {
        let value = decimal_ipv4_octet(segment)?;
        if value > 255 {
            return None;
        }
        if !normalized.is_empty() {
            normalized.push('.');
        }
        write!(&mut normalized, "{value}").ok()?;
    }
    Some(normalized)
}

/// Разбирает Snum только с 1..3 ASCII цифрами.
fn decimal_ipv4_octet(octet: &str) -> Option<u16> {
    match (1..=3).contains(&octet.len()) {
        true if octet.bytes().all(|byte| byte.is_ascii_digit()) => octet.parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
#[path = "address_literal_tests.rs"]
mod tests;
