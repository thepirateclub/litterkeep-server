//! Владение ресурсами одной TCP-сессии reactor-а.
use super::*;
use std::time::{Duration, Instant};

/// Максимальное время, на которое TLS-клиент может занять slot без handshake.
const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Состояние SMTP transaction, принадлежащее только соответствующей session.
pub(super) enum SmtpState {
    Command(smtp::Transaction),
    Data {
        bytes: Vec<u8>,
        recipients: Vec<String>,
    },
}

/// Все буферы, аутентификация и TLS transport одного клиентского владельца.
pub(super) struct Session {
    pub(super) generation: u64,
    pub(super) kind: Kind,
    pub(super) stream: TcpStream,
    pub(super) input: Vec<u8>,
    pub(super) output: Vec<u8>,
    pub(super) wire_output: Vec<u8>,
    pub(super) tls: Option<ServerTls>,
    starttls_config: Option<Arc<ServerConfig>>,
    starttls_pending: bool,
    handshake_deadline: Option<Instant>,
    greeting_sent: bool,
    pub(super) pending_submit: Option<(Request, Pending)>,
    pub(super) pending: Option<Pending>,
    pub(super) authenticated_account: Option<String>,
    pub(super) smtp: SmtpState,
}

impl Session {
    pub(super) fn new(
        kind: Kind,
        stream: TcpStream,
        generation: u64,
        tls_config: Option<Arc<ServerConfig>>,
        starttls_config: Option<Arc<ServerConfig>>,
    ) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        let tls = tls_config
            .map(ServerTls::new)
            .transpose()
            .map_err(io::Error::other)?;
        let handshake_deadline = tls.as_ref().map(|_| Instant::now() + TLS_HANDSHAKE_TIMEOUT);
        let mut result = Self {
            generation,
            kind,
            stream,
            input: Vec::new(),
            output: Vec::new(),
            wire_output: Vec::new(),
            tls,
            starttls_config,
            starttls_pending: false,
            handshake_deadline,
            greeting_sent: false,
            pending_submit: None,
            pending: None,
            authenticated_account: None,
            smtp: SmtpState::Command(smtp::Transaction::default()),
        };
        result.send_greeting_if_ready()?;
        Ok(result)
    }

    /// Добавляет greeting лишь plaintext-сессии либо завершившему handshake клиенту.
    pub(super) fn send_greeting_if_ready(&mut self) -> io::Result<()> {
        if self.greeting_sent {
            return Ok(());
        }
        if self.tls.as_ref().is_some_and(ServerTls::is_handshaking) {
            return Ok(());
        }
        self.handshake_deadline = None;
        let greeting = match self.kind {
            Kind::Smtp => b"220 litterkeep.test ESMTP litterkeep-server\r\n".as_slice(),
            Kind::Imap => b"* OK IMAP4rev1 litterkeep-server\r\n".as_slice(),
        };
        self.push(greeting)?;
        self.greeting_sent = true;
        Ok(())
    }

    pub(super) fn push(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.output.len().saturating_add(bytes.len()) > MAX_OUTPUT {
            return Err(io::Error::other("session output limit"));
        }
        self.output.extend_from_slice(bytes);
        Ok(())
    }

    fn accepts_input(&self) -> bool {
        match (self.pending.is_some(), self.pending_submit.is_some()) {
            (true, _) | (_, true) => false,
            (false, false) => self.output.len() < MAX_OUTPUT,
        }
    }

    pub(super) fn needs_read(&self) -> bool {
        match self.tls.as_ref() {
            Some(tls) if tls.wants_read() => true,
            _ => self.accepts_input(),
        }
    }

    pub(super) fn needs_write(&self) -> bool {
        if !self.wire_output.is_empty() {
            return true;
        }
        match self.tls.as_ref() {
            Some(tls) if tls.wants_write() => true,
            Some(tls) => match (tls.is_handshaking(), self.output.is_empty()) {
                (false, false) => true,
                (true, _) | (false, true) => false,
            },
            None => !self.output.is_empty(),
        }
    }

    /// Возвращает deadline только пока rustls действительно выполняет handshake.
    pub(super) fn handshake_deadline(&self) -> Option<Instant> {
        match self.tls.as_ref() {
            Some(tls) if tls.is_handshaking() => self.handshake_deadline,
            Some(_) | None => None,
        }
    }

    /// Отделяет истечение handshake от обычного idle состояния протокола.
    pub(super) fn handshake_expired(&self, now: Instant) -> bool {
        self.handshake_deadline()
            .is_some_and(|deadline| deadline <= now)
    }

    /// Сообщает SMTP FSM, доступен ли переход из plaintext в TLS.
    pub(super) fn can_starttls(&self) -> bool {
        matches!(
            (
                self.tls.is_none(),
                self.starttls_config.is_some(),
                self.starttls_pending
            ),
            (true, true, false)
        )
    }

    /// Планирует upgrade после полной отправки plaintext-ответа `220`.
    pub(super) fn request_starttls(&mut self) {
        self.input.clear();
        self.starttls_pending = true;
        self.smtp = SmtpState::Command(smtp::Transaction::default());
        self.authenticated_account = None;
    }

    /// Передаёт socket новому rustls handshake только на чистой границе вывода.
    pub(super) fn begin_starttls_if_ready(&mut self) -> io::Result<()> {
        if !matches!(
            (self.starttls_pending, self.output.is_empty()),
            (true, true)
        ) {
            return Ok(());
        }
        let config = self
            .starttls_config
            .take()
            .ok_or_else(|| io::Error::other("STARTTLS config missing"))?;
        self.tls = Some(ServerTls::new(config).map_err(io::Error::other)?);
        self.starttls_pending = false;
        self.handshake_deadline = Some(Instant::now() + TLS_HANDSHAKE_TIMEOUT);
        Ok(())
    }
}
