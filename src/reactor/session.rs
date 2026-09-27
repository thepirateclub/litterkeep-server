//! Владение ресурсами одной TCP-сессии reactor-а.
use super::*;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

#[path = "session_shutdown.rs"]
mod shutdown;
#[path = "session_transport_methods.rs"]
mod transport_methods;
pub(in crate::reactor) use shutdown::SessionTermination;

/// Максимальное время, на которое TLS-клиент может занять slot без handshake.
const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Состояние SMTP transaction, принадлежащее только соответствующей session.
pub(super) enum SmtpState {
    Command(smtp::Transaction),
    Data {
        bytes: Vec<u8>,
        section: DataSection,
        recipients: Vec<String>,
        reverse_path: String,
        oversized: bool,
        invalid_octets: bool,
        body_mode: crate::smtp::BodyMode,
        smtp_utf8: bool,
    },
    /// Дренирует BDAT от peer-а, которому сервер не объявлял CHUNKING.
    BdatDiscard {
        receiver: super::smtp_bdat_chunk::Receiver,
    },
    /// Между чанками сохраняет один envelope и накопленные message octets.
    BdatReady(super::smtp_bdat::Transaction),
    /// После отклонённого BDAT принимает только RSET и дренирует chunks из pipeline.
    BdatFailed,
    /// Получает следующий BDAT payload ровно по объявленному размеру.
    BdatReceiving(super::smtp_bdat::Chunk),
}

/// Стриминговое состояние DATA-парсера до и после пустой строки заголовков.
#[derive(Clone, Copy)]
pub(super) enum DataSection {
    Headers,
    Body,
}

/// Явно хранит PLAIN challenge до ответа или отмены со стороны клиента.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum SmtpAuthState {
    #[default]
    Command,
    AwaitingPlainResponse,
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
    /// Последняя передача либо получение TCP octets для SMTP idle timer.
    pub(super) last_activity: Instant,
    /// Начало ожидания текущего DATA-блока или незавершённой DATA-строки.
    pub(super) smtp_data_activity: Instant,
    /// Отмечает, что TLS close_notify уже передан rustls ровно один раз.
    pub(super) close_notify_started: bool,
    greeting_sent: bool,
    pub(super) pending_submit: Option<(Request, Pending)>,
    pub(super) pending: Option<Pending>,
    pub(super) authenticated_account: Option<String>,
    /// IMAP string grammar выбранной для этой сессии договорённостью.
    pub(super) imap_string_mode: imap::tokenizer::StringMode,
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
    /// IMAP literal ожидает завершения неблокирующего приёма octets.
    pub(super) imap_search_literal: Option<super::imap_search_literal::Receiver>,
    /// Сохранённые октеты literal-аргументов для типизированного parser-а команды.
    pub(super) imap_search_literal_value: Option<Vec<Vec<u8>>>,
    pub(super) smtp: SmtpState,
    pub(super) smtp_auth: SmtpAuthState,
    pub(super) smtp_greeting: smtp::GreetingMode,
    pub(super) smtp_body_mode: crate::smtp::BodyMode,
    /// Маркер SMTPUTF8 действует только для envelope текущей транзакции.
    pub(super) smtp_utf8: bool,
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
    /// Возвращает RECENT из выбранной сессии либо ещё не claimed Store UIDs.
    pub(super) fn recent_count_for(&self, mailbox_key: &str, unclaimed: usize) -> usize {
        match self.selected_mailbox.as_deref() {
            Some(selected) if selected == mailbox_key => self.selected_recent.len(),
            _ => unclaimed,
        }
    }

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
            last_activity: Instant::now(),
            smtp_data_activity: Instant::now(),
            close_notify_started: false,
            greeting_sent: false,
            pending_submit: None,
            pending: None,
            authenticated_account: None,
            imap_string_mode: imap::tokenizer::StringMode::default(),
            selected_mailbox: None,
            selected_exists: 0,
            selected_recent: BTreeSet::new(),
            saved_search_uids: BTreeSet::new(),
            selected_read_only: false,
            imap_idle: None,
            imap_authenticate: None,
            imap_search_literal: None,
            imap_search_literal_value: None,
            smtp: SmtpState::Command(smtp::Transaction::default()),
            smtp_auth: SmtpAuthState::Command,
            smtp_greeting: smtp::GreetingMode::NotEstablished,
            smtp_body_mode: crate::smtp::BodyMode::SevenBit,
            smtp_utf8: false,
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
            Kind::Smtp => b"220 litterkeep.test ESMTP litterkeep-server\r\n".to_vec(),
            Kind::Imap => b"* OK IMAP4rev1 litterkeep-server\r\n".to_vec(),
            Kind::Pop3 => b"+OK litterkeep-server ready\r\n".to_vec(),
            Kind::ManageSieve => {
                crate::managesieve::greeting(self.is_encrypted(), self.can_starttls())
            }
        };
        self.push(&greeting)?;
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
}

#[path = "session_termination.rs"]
pub(super) mod termination;
