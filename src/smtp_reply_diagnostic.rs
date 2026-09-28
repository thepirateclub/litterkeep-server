//! Безопасный типизированный SMTP diagnostic для долговечной DSN.

use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

/// Последний полный отказ SMTP peer, подходящий для поля Diagnostic-Code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmtpReplyDiagnostic {
    pub(crate) code: u16,
    pub(crate) text: String,
    pub(crate) remote_mta: Option<RemoteMtaIdentity>,
}

/// Идентичность удалённого MTA берётся из TLS server name либо socket route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RemoteMtaIdentity {
    Dns(String),
    Ipv4(Ipv4Addr),
    Ipv6(Ipv6Addr),
}

impl RemoteMtaIdentity {
    pub(crate) fn from_ip(address: IpAddr) -> Self {
        match address {
            IpAddr::V4(value) => Self::Ipv4(value),
            IpAddr::V6(value) => Self::Ipv6(value),
        }
    }

    pub(crate) fn from_server_name(value: &str) -> Self {
        match value.parse::<IpAddr>() {
            Ok(address) => Self::from_ip(address),
            Err(_) => Self::Dns(value.to_owned()),
        }
    }

    fn journal_token(&self) -> String {
        let (tag, bytes) = match self {
            Self::Dns(value) => ('D', value.as_bytes().to_vec()),
            Self::Ipv4(value) => ('4', value.to_string().into_bytes()),
            Self::Ipv6(value) => ('6', value.to_string().into_bytes()),
        };
        format!("{tag}{}", encode_hex(&bytes))
    }

    fn from_journal_token(token: &str) -> io::Result<Self> {
        let (tag, encoded) = token
            .split_at_checked(1)
            .ok_or_else(|| invalid("remote MTA identity tag is missing"))?;
        let bytes = decode_hex(encoded)?;
        let value = String::from_utf8(bytes).map_err(|_| invalid("remote MTA is not UTF-8"))?;
        match tag {
            "D" if valid_dns_name(&value) => Ok(Self::Dns(value)),
            "4" => value
                .parse::<Ipv4Addr>()
                .map(Self::Ipv4)
                .map_err(|_| invalid("remote IPv4 MTA is invalid")),
            "6" => value
                .parse::<Ipv6Addr>()
                .map(Self::Ipv6)
                .map_err(|_| invalid("remote IPv6 MTA is invalid")),
            _ => Err(invalid("remote MTA identity is invalid")),
        }
    }

    pub(crate) fn dsn_value(&self) -> String {
        match self {
            Self::Dns(value) => value.clone(),
            Self::Ipv4(value) => format!("[{value}]"),
            Self::Ipv6(value) => format!("[IPv6:{value}]"),
        }
    }
}

impl SmtpReplyDiagnostic {
    /// Проверяет SMTP-класс и ограничивает текст одним безопасным journal token.
    pub(crate) fn new(code: u16, text: String) -> io::Result<Self> {
        match (400..600).contains(&code)
            && text.len() <= 16_384
            && !text.bytes().any(|byte| byte.is_ascii_control())
        {
            true => Ok(Self {
                code,
                text,
                remote_mta: None,
            }),
            false => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid SMTP diagnostic fields",
            )),
        }
    }

    /// Привязывает ответ сервера к тому MTA, чей SMTP transaction его вернул.
    pub(crate) fn with_remote_mta(mut self, remote_mta: RemoteMtaIdentity) -> Self {
        self.remote_mta = Some(remote_mta);
        self
    }

    /// Кодирует текст в hex, чтобы пробелы ответа не меняли поля журнала.
    pub(crate) fn journal_token(&self) -> String {
        let mut token = format!("{}:", self.code);
        for byte in self.text.as_bytes() {
            use std::fmt::Write as _;
            let _ = write!(token, "{byte:02X}");
        }
        if let Some(remote_mta) = self.remote_mta.as_ref() {
            use std::fmt::Write as _;
            let _ = write!(token, ":{}", remote_mta.journal_token());
        }
        token
    }

    /// Возвращает только синтаксически корректный enhanced status того же класса.
    pub(crate) fn enhanced_status_code(&self) -> Option<&str> {
        let value = self.text.split_whitespace().next()?;
        let mut components = value.split('.');
        let (Some(class), Some(subject), Some(detail), None) = (
            components.next(),
            components.next(),
            components.next(),
            components.next(),
        ) else {
            return None;
        };
        let class = parse_status_component(class)?;
        parse_status_component(subject)?;
        parse_status_component(detail)?;
        let class = match class {
            2 => EnhancedStatusClass::Success,
            4 => EnhancedStatusClass::Transient,
            5 => EnhancedStatusClass::Permanent,
            _ => return None,
        };
        match class.smtp_class() == self.code / 100 {
            true => Some(value),
            false => None,
        }
    }

    /// Восстанавливает ограниченную запись, не принимая повреждённый hex.
    pub(crate) fn from_journal_token(token: &str) -> io::Result<Self> {
        let (code, encoded) = token
            .split_once(':')
            .ok_or_else(|| invalid("SMTP diagnostic separator is missing"))?;
        let (encoded, remote_token) = match encoded.split_once(':') {
            Some((text, remote)) => (text, Some(remote)),
            None => (encoded, None),
        };
        let code = code
            .parse::<u16>()
            .map_err(|_| invalid("SMTP diagnostic code is invalid"))?;
        match (encoded.len() % 2, encoded.len() <= 32_768) {
            (0, true) => {}
            _ => return Err(invalid("SMTP diagnostic hex size is invalid")),
        }
        let mut bytes = Vec::with_capacity(encoded.len() / 2);
        let mut octets = encoded.bytes();
        while let Some(high) = octets.next() {
            let low = octets
                .next()
                .ok_or_else(|| invalid("odd SMTP diagnostic hex"))?;
            bytes.push((hex(high)? << 4) | hex(low)?);
        }
        let text = String::from_utf8(bytes).map_err(|_| invalid("SMTP diagnostic is not UTF-8"))?;
        let mut diagnostic = Self::new(code, text)?;
        diagnostic.remote_mta = remote_token
            .map(RemoteMtaIdentity::from_journal_token)
            .transpose()?;
        Ok(diagnostic)
    }
}

/// Классы enhanced status, сопоставляемые с SMTP reply code.
enum EnhancedStatusClass {
    Success,
    Transient,
    Permanent,
}

impl EnhancedStatusClass {
    fn smtp_class(&self) -> u16 {
        match self {
            Self::Success => 2,
            Self::Transient => 4,
            Self::Permanent => 5,
        }
    }
}

fn parse_status_component(value: &str) -> Option<u16> {
    match !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
        true => value.parse().ok(),
        false => None,
    }
}

fn hex(byte: u8) -> io::Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(invalid("SMTP diagnostic contains invalid hex")),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        let _ = write!(output, "{byte:02X}");
    }
    output
}

fn decode_hex(encoded: &str) -> io::Result<Vec<u8>> {
    match (encoded.len() % 2, encoded.len() <= 32_768) {
        (0, true) => {}
        _ => return Err(invalid("remote MTA hex size is invalid")),
    }
    let mut bytes = Vec::with_capacity(encoded.len() / 2);
    let mut octets = encoded.bytes();
    while let Some(high) = octets.next() {
        let low = octets.next().ok_or_else(|| invalid("odd remote MTA hex"))?;
        bytes.push((hex(high)? << 4) | hex(low)?);
    }
    Ok(bytes)
}

fn valid_dns_name(value: &str) -> bool {
    match (value.is_empty(), value.len() <= 255) {
        (false, true) => value.split('.').all(valid_dns_label),
        _ => false,
    }
}

fn valid_dns_label(label: &str) -> bool {
    let mut bytes = label.bytes();
    match (label.len() <= 63, bytes.next(), label.as_bytes().last()) {
        (true, Some(first), Some(last)) => {
            first.is_ascii_alphanumeric()
                && last.is_ascii_alphanumeric()
                && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        }
        _ => false,
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
#[path = "smtp_reply_diagnostic_tests.rs"]
mod tests;
