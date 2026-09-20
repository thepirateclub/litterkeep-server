//! Узкий blocking IMAP-over-TLS клиент только для локального seed workflow.

use rustls::{ClientConnection, StreamOwned, pki_types::ServerName};
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;

type TlsStream = StreamOwned<ClientConnection, TcpStream>;

/// Результат tagged IMAP response, разобранный на протокольной границе.
enum TaggedStatus {
    Ok,
    No,
    Bad,
}

pub(super) struct Client {
    stream: BufReader<TlsStream>,
    next_tag: u64,
}

impl Client {
    pub(super) fn connect(
        address: &str,
        server_name: &str,
        ca_der: &[u8],
        account: &str,
        password: &[u8],
    ) -> io::Result<Self> {
        let config = crate::outbound_tls::client_config(ca_der)?;
        let name = ServerName::try_from(server_name.to_owned())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid TLS server name"))?;
        let tcp = crate::outbound::connect_bounded(address)?;
        let tls = ClientConnection::new(config, name).map_err(io::Error::other)?;
        let mut result = Self {
            stream: BufReader::new(StreamOwned::new(tls, tcp)),
            next_tag: 1,
        };
        result.expect_greeting()?;
        result.command(&format!(
            "LOGIN {} {}",
            quote(account),
            quote_bytes(password)
        ))?;
        Ok(result)
    }

    /// Ищет fixture только в выбранной папке по закрытому seed-заголовку.
    pub(super) fn contains(&mut self, mailbox: &str, fixture_id: &str) -> io::Result<bool> {
        self.command(&format!("SELECT {}", quote(mailbox)))?;
        let lines = self.command(&format!(
            "SEARCH HEADER X-Litterkeep-Fixture {}",
            quote(fixture_id)
        ))?;
        Ok(lines.iter().any(|line| search_has_result(line)))
    }

    /// Выполняет synchronizing literal APPEND и ждёт durable tagged OK.
    pub(super) fn append(&mut self, mailbox: &str, flags: &str, message: &[u8]) -> io::Result<()> {
        let tag = self.tag();
        let flag_list = match flags.is_empty() {
            true => String::new(),
            false => format!(" ({flags})"),
        };
        self.write(format!(
            "{tag} APPEND {}{flag_list} {{{}}}\r\n",
            quote(mailbox),
            message.len()
        ))?;
        let continuation = self.read_line()?;
        if !continuation.starts_with('+') {
            return Err(io::Error::other("IMAP APPEND continuation missing"));
        }
        self.stream.get_mut().write_all(message)?;
        self.stream.get_mut().write_all(b"\r\n")?;
        self.stream.get_mut().flush()?;
        self.read_tagged(&tag).map(|_| ())
    }

    fn command(&mut self, command: &str) -> io::Result<Vec<String>> {
        let tag = self.tag();
        self.write(format!("{tag} {command}\r\n"))?;
        self.read_tagged(&tag)
    }

    fn tag(&mut self) -> String {
        let tag = format!("S{}", self.next_tag);
        self.next_tag = self.next_tag.wrapping_add(1).max(1);
        tag
    }

    fn write(&mut self, wire: String) -> io::Result<()> {
        self.stream.get_mut().write_all(wire.as_bytes())?;
        self.stream.get_mut().flush()
    }

    fn expect_greeting(&mut self) -> io::Result<()> {
        match self.read_line()?.starts_with("* OK") {
            true => Ok(()),
            false => Err(io::Error::other("invalid IMAP greeting")),
        }
    }

    fn read_tagged(&mut self, tag: &str) -> io::Result<Vec<String>> {
        let mut lines = Vec::new();
        loop {
            let line = self.read_line()?;
            if let Some(status) = tagged_status(&line, tag) {
                return match status {
                    TaggedStatus::Ok => Ok(lines),
                    TaggedStatus::No => Err(io::Error::other("IMAP command rejected")),
                    TaggedStatus::Bad => Err(io::Error::other("IMAP command invalid")),
                };
            }
            lines.push(line);
        }
    }

    fn read_line(&mut self) -> io::Result<String> {
        let mut line = String::new();
        if self.stream.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "IMAP peer closed",
            ));
        }
        Ok(line)
    }
}

fn tagged_status(line: &str, expected_tag: &str) -> Option<TaggedStatus> {
    let mut fields = line.split_whitespace();
    match (fields.next(), fields.next()) {
        (Some(tag), Some(status)) if tag == expected_tag => match status {
            "OK" => Some(TaggedStatus::Ok),
            "NO" => Some(TaggedStatus::No),
            "BAD" => Some(TaggedStatus::Bad),
            _ => Some(TaggedStatus::Bad),
        },
        _ => None,
    }
}

fn search_has_result(line: &str) -> bool {
    let mut fields = line.split_whitespace();
    matches!(
        (fields.next(), fields.next(), fields.next()),
        (Some("*"), Some("SEARCH"), Some(_))
    )
}

fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn quote_bytes(value: &[u8]) -> String {
    quote(std::str::from_utf8(value).unwrap_or(""))
}
