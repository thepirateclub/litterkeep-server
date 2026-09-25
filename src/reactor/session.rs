//! Владение ресурсами одной TCP-сессии reactor-а.
use super::*;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

#[path = "session_shutdown.rs"]
mod shutdown;
pub(in crate::reactor) use shutdown::SessionTermination;

/// Максимальное время, на которое TLS-клиент может занять slot без handshake.
const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Состояние SMTP transaction, принадлежащее только соответствующей session.
pub(super) enum SmtpState {
    Command(smtp::Transaction),
    Data {
        bytes: Vec<u8>,
        recipients: Vec<String>,
        reverse_path: String,
        oversized: bool,
        body_mode: crate::smtp::BodyMode,
    },
}

/// DATA принимает длинные тела, но команды остаются ограниченными одним MiB.
fn session_input_limit(session: &Session) -> usize {
    match (&session.kind, &session.smtp) {
        (Kind::Smtp, SmtpState::Data { .. }) => MAX_MESSAGE + MAX_INPUT,
        _ => MAX_INPUT,
    }
}

/// Все буферы, аутентификация и TLS transport одного клиентского владельца.
pub(super) struct Session {
    pub(super) generation: u64,
    pub(super) kind: Kind,
    pub(super) stream: TcpStream,
    pub(super) input: Vec<u8>,
    pub(super) output: Vec<u8>,
    pub(super) wire_output: Vec<u8>,
    /// Причина локального завершения, не являющаяся ошибкой всего reactor-а.
    pub(super) termination: Option<SessionTermination>,
    pub(super) tls: Option<ServerTls>,
    starttls_config: Option<Arc<ServerConfig>>,
    starttls_pending: bool,
    handshake_deadline: Option<Instant>,
    /// Отмечает, что TLS close_notify уже передан rustls ровно один раз.
    pub(super) close_notify_started: bool,
    greeting_sent: bool,
    pub(super) pending_submit: Option<(Request, Pending)>,
    pub(super) pending: Option<Pending>,
    pub(super) authenticated_account: Option<String>,
    /// Store key выбранного mailbox; `None` означает authenticated state.
    pub(super) selected_mailbox: Option<String>,
    pub(super) selected_exists: usize,
    /// `\\Recent` принадлежит session, а не persistent MessageFlags.
    pub(super) selected_recent: BTreeSet<u64>,
    /// UIDs последнего успешного SEARCH RETURN (SAVE), только для этой сессии.
    pub(super) saved_search_uids: BTreeSet<u64>,
    /// EXAMINE запрещает все изменяющие выбранный mailbox команды.
    pub(super) selected_read_only: bool,
    /// Активный IDLE сохраняет session lease и не создаёт worker/thread.
    pub(super) imap_idle: Option<IdleState>,
    /// SASL continuation остаётся у socket owner до ответа либо отмены.
    pub(super) imap_authenticate: Option<imap_protocol::authenticate::State>,
    /// Ограниченный literal остаётся собственностью этой IMAP-сессии.
    pub(super) imap_literal: Option<LiteralReceiver>,
    /// SEARCH literal временно заменяется quoted UTF-8 строкой после приёма.
    pub(super) imap_search_literal: Option<super::imap_search_literal::Receiver>,
    pub(super) smtp: SmtpState,
    pub(super) smtp_greeting: smtp::GreetingMode,
    pub(super) smtp_body_mode: crate::smtp::BodyMode,
    /// POP3 хранит snapshot только до QUIT и не получает Store во владение.
    pub(super) pop3: pop3::Session,
    /// Внутренний ключ INBOX нужен для durable применения POP3 DELE на QUIT.
    pub(super) pop3_mailbox: Option<String>,
    /// ManageSieve FSM не владеет script catalog и ждёт worker completion.
    pub(super) managesieve: managesieve::Session,
    /// Script literal принадлежит ровно одной ManageSieve session до completion.
    pub(super) managesieve_literal: Option<managesieve_literal::LiteralReceiver>,
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
            termination: None,
            tls,
            starttls_config,
            starttls_pending: false,
            handshake_deadline,
            close_notify_started: false,
            greeting_sent: false,
            pending_submit: None,
            pending: None,
            authenticated_account: None,
            selected_mailbox: None,
            selected_exists: 0,
            selected_recent: BTreeSet::new(),
            saved_search_uids: BTreeSet::new(),
            selected_read_only: false,
            imap_idle: None,
            imap_authenticate: None,
            imap_literal: None,
            imap_search_literal: None,
            smtp: SmtpState::Command(smtp::Transaction::default()),
            smtp_greeting: smtp::GreetingMode::NotEstablished,
            smtp_body_mode: crate::smtp::BodyMode::SevenBit,
            pop3: pop3::Session::default(),
            pop3_mailbox: None,
            managesieve: managesieve::Session::default(),
            managesieve_literal: None,
        };
        if matches!((result.kind, result.tls.is_some()), (Kind::Pop3, true)) {
            result.pop3.tls_established();
        }
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
            Kind::Pop3 => b"+OK litterkeep-server ready\r\n".as_slice(),
            Kind::ManageSieve => b"\"IMPLEMENTATION\" \"litterkeep-server\"\r\n\"SASL\" \"PLAIN\"\r\n\"STARTTLS\"\r\n\"SIEVE\" \"fileinto\"\r\nOK \"ready\"\r\n".as_slice(),
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

    /// Добавляет unsolicited IMAP-событие, не позволяя медленному подписчику
    /// исчерпать память или завершить общий серверный цикл.
    ///
    /// При переполнении сессия теряет ещё не отправленные события и помечается
    /// для закрытия единым путём освобождения lease. Клиент обязан выполнить
    /// повторный SELECT после соединения, поэтому частичная sequence-проекция
    /// не выдаётся за актуальную.
    pub(super) fn push_notification(&mut self, bytes: &[u8]) {
        let next_size = self.output.len().saturating_add(bytes.len());
        if next_size <= MAX_OUTPUT {
            self.output.extend_from_slice(bytes);
            return;
        }
        self.output.clear();
        self.wire_output.clear();
        self.termination = Some(SessionTermination::NotificationOverflow);
    }

    fn accepts_input(&self) -> bool {
        if self.termination.is_some() {
            return false;
        }
        match (self.pending.is_some(), self.pending_submit.is_some()) {
            (true, _) | (_, true) => false,
            (false, false) => self.output.len() < MAX_OUTPUT,
        }
    }

    /// Ограничивает командные буферы; DATA потоково обрабатывается построчно.
    pub(super) fn input_limit_exceeded(&self) -> bool {
        self.input.len() > session_input_limit(self)
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

    /// Парольные механизмы разрешены только после реального TLS transport-а.
    pub(super) fn is_encrypted(&self) -> bool {
        self.tls.is_some()
    }

    /// Планирует upgrade после полной отправки plaintext-ответа `220`.
    pub(super) fn request_starttls(&mut self) {
        self.input.clear();
        self.starttls_pending = true;
        self.smtp = SmtpState::Command(smtp::Transaction::default());
        self.smtp_greeting = smtp::GreetingMode::NotEstablished;
        self.smtp_body_mode = crate::smtp::BodyMode::SevenBit;
        self.authenticated_account = None;
        self.selected_mailbox = None;
        self.selected_exists = 0;
        self.selected_recent.clear();
        self.imap_idle = None;
        self.imap_authenticate = None;
        self.imap_literal = None;
        self.imap_search_literal = None;
        self.pop3 = pop3::Session::default();
        if matches!(self.kind, Kind::Pop3) {
            self.pop3.tls_established();
        }
        self.pop3_mailbox = None;
        self.managesieve_literal = None;
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

#[path = "session_termination.rs"]
pub(super) mod termination;
