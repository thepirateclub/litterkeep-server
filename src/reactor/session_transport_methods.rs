//! Методы TLS lifecycle для сессии, владеющей клиентским socket.

use super::*;

impl Session {
    /// Возвращает deadline только пока rustls действительно выполняет handshake.
    pub(in crate::reactor) fn handshake_deadline(&self) -> Option<Instant> {
        match self.tls.as_ref() {
            Some(tls) if tls.is_handshaking() => self.handshake_deadline,
            Some(_) | None => None,
        }
    }

    /// Отделяет истечение handshake от обычного idle состояния протокола.
    pub(in crate::reactor) fn handshake_expired(&self, now: Instant) -> bool {
        self.handshake_deadline()
            .is_some_and(|deadline| deadline <= now)
    }

    /// Сообщает SMTP FSM, доступен ли переход из plaintext в TLS.
    pub(in crate::reactor) fn can_starttls(&self) -> bool {
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
    pub(in crate::reactor) fn is_encrypted(&self) -> bool {
        self.tls.is_some()
    }

    /// Планирует upgrade после полной отправки plaintext-ответа `220`.
    pub(in crate::reactor) fn request_starttls(&mut self) {
        self.input.clear();
        if matches!(self.kind, Kind::ManageSieve) {
            self.greeting_sent = false;
        }
        self.starttls_pending = true;
        self.smtp = SmtpState::Command(smtp::Transaction::default());
        self.smtp_auth = SmtpAuthState::Command;
        self.smtp_greeting = smtp::GreetingMode::NotEstablished;
        self.smtp_body_mode = crate::smtp::BodyMode::SevenBit;
        self.smtp_utf8 = false;
        self.imap_string_mode = imap::tokenizer::StringMode::Legacy;
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
    pub(in crate::reactor) fn begin_starttls_if_ready(&mut self) -> io::Result<()> {
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
