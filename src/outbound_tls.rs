//! Исходящий SMTP поверх TLS с явной проверкой локально заданного CA и SNI.
//!
//! Это standalone transport для route, где policy требует implicit TLS. Он не
//! разделяет plaintext pool: TLS identity является отдельным ключом ресурса.
use crate::outbound::{self, DeliveryOutcome};
#[path = "outbound_tls_delivery.rs"]
mod delivery;
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, StreamOwned,
    pki_types::{CertificateDer, ServerName},
};
use std::{
    io::{self, BufReader},
    net::TcpStream,
    sync::Arc,
};

/// Одно implicit-TLS SMTP-соединение с закреплёнными CA и SNI context.
pub struct Connection {
    stream: BufReader<StreamOwned<ClientConnection, TcpStream>>,
    capabilities: outbound::EhloCapabilities,
    close_after_response: bool,
    timeouts: outbound::SmtpTimeouts,
    last_diagnostic: Option<crate::smtp_reply_diagnostic::SmtpReplyDiagnostic>,
}

impl Connection {
    /// Устанавливает TCP/TLS канал и проверяет SMTP greeting с EHLO.
    pub fn connect(address: &str, server_name: &str, ca_der: &[u8]) -> io::Result<Self> {
        let timeouts = outbound::SmtpTimeouts::from_environment()?;
        Self::connect_with_timeouts(address, server_name, ca_der, timeouts)
    }

    fn connect_with_timeouts(
        address: &str,
        server_name: &str,
        ca_der: &[u8],
        timeouts: outbound::SmtpTimeouts,
    ) -> io::Result<Self> {
        let config = client_config(ca_der)?;
        let name = ServerName::try_from(server_name.to_owned())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid TLS server name"))?;
        let stream = outbound::connect_bounded_with_timeouts(address, timeouts)?;
        let connection = ClientConnection::new(config, name).map_err(io::Error::other)?;
        let mut result = Self {
            stream: BufReader::new(StreamOwned::new(connection, stream)),
            capabilities: outbound::EhloCapabilities::default(),
            close_after_response: false,
            timeouts,
            last_diagnostic: None,
        };
        expect_success(&mut result.stream, outbound::ReplyExpectation::Greeting)?;
        // TLS EHLO является обычной командой после отдельного greeting deadline.
        result
            .stream
            .get_ref()
            .sock
            .set_read_timeout(Some(result.timeouts.command()))?;
        result.command("EHLO litterkeep.test\r\n")?;
        match outbound::negotiate_ehlo(&mut result.stream)? {
            outbound::EhloNegotiation::Extended(capabilities) => result.capabilities = capabilities,
            outbound::EhloNegotiation::Basic => {
                result.command("HELO litterkeep.test\r\n")?;
                expect_success(&mut result.stream, outbound::ReplyExpectation::Helo)?;
            }
        }
        Ok(result)
    }

    #[cfg(test)]
    pub(crate) fn connect_for_timeout_test(
        address: &str,
        server_name: &str,
        ca_der: &[u8],
        timeouts: outbound::SmtpTimeouts,
    ) -> io::Result<Self> {
        Self::connect_with_timeouts(address, server_name, ca_der, timeouts)
    }

    /// Аутентифицирует локальный submission-клиент механизмом SASL PLAIN.
    pub fn authenticate(&mut self, account: &str, password: &[u8]) -> io::Result<()> {
        let response = crate::sasl::encode_plain(account, password);
        self.command(&format!("AUTH PLAIN {response}\r\n"))?;
        self.expect_success(outbound::ReplyExpectation::Authentication)
    }

    /// Доказывает возврат TLS connection в чистое SMTP состояние.
    pub fn reset(&mut self) -> io::Result<()> {
        self.command("RSET\r\n")?;
        self.expect_success(outbound::ReplyExpectation::Reset)
    }

    /// Завершает отдельную TLS SMTP session командой QUIT.
    pub fn quit(&mut self) -> io::Result<()> {
        self.command("QUIT\r\n")?;
        self.expect_success(outbound::ReplyExpectation::Quit)
    }

    fn command(&mut self, wire: &str) -> io::Result<()> {
        self.stream
            .get_ref()
            .sock
            .set_write_timeout(Some(self.timeouts.command()))?;
        command(&mut self.stream, wire)
    }

    /// Даёт TLS pool знать, что peer потребовал закрыть канал после reply.
    pub(crate) fn should_close(&self) -> bool {
        self.close_after_response
    }

    /// Копирует diagnostic до выдачи TLS connection следующему job.
    pub(crate) fn last_diagnostic(
        &self,
    ) -> Option<crate::smtp_reply_diagnostic::SmtpReplyDiagnostic> {
        self.last_diagnostic.clone()
    }

    fn expect_command(
        &mut self,
        expectation: outbound::ReplyExpectation,
    ) -> io::Result<Option<DeliveryOutcome>> {
        self.stream
            .get_ref()
            .sock
            .set_read_timeout(Some(self.timeouts.response(expectation)))?;
        let (reply, diagnostic) = outbound::expect_with_diagnostic(&mut self.stream, expectation)?;
        if diagnostic.is_some() {
            self.last_diagnostic = diagnostic;
        }
        match reply {
            outbound::ReplyResult::Accepted => Ok(None),
            outbound::ReplyResult::Rejected(outcome) => Ok(Some(outcome)),
            outbound::ReplyResult::Closing => {
                self.close_after_response = true;
                Ok(Some(DeliveryOutcome::Deferred))
            }
        }
    }

    fn expect_success(&mut self, expectation: outbound::ReplyExpectation) -> io::Result<()> {
        match self.expect_command(expectation)? {
            None => Ok(()),
            Some(DeliveryOutcome::Deferred) => Err(io::Error::other("TLS SMTP peer deferred")),
            Some(DeliveryOutcome::PermanentFailure) => {
                Err(io::Error::other("TLS SMTP peer rejected"))
            }
            Some(DeliveryOutcome::Delivered | DeliveryOutcome::Relayed) => {
                Err(io::Error::other("unexpected successful SMTP outcome"))
            }
        }
    }
}

/// Выполняет одну implicit-TLS транзакцию, проверяет CA/SNI и завершает session командой QUIT.
///
/// Ошибка QUIT не отменяет уже подтверждённый SMTP результат; канал, закрываемый
/// peer ответом 421, не получает дополнительной команды.
pub fn deliver(
    address: &str,
    server_name: &str,
    ca_der: &[u8],
    sender: &str,
    recipient: &str,
    message: &[u8],
) -> io::Result<DeliveryOutcome> {
    let mut connection = Connection::connect(address, server_name, ca_der)?;
    let outcome = connection.deliver(sender, recipient, message)?;
    if !connection.should_close() {
        let _ = connection.quit();
    }
    Ok(outcome)
}

pub(crate) fn client_config(ca_der: &[u8]) -> io::Result<Arc<ClientConfig>> {
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from(ca_der.to_vec()))
        .map_err(io::Error::other)?;
    Ok(Arc::new(
        ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    ))
}

#[path = "outbound_tls_command.rs"]
mod tls_command;
use tls_command::{command, expect_success};
