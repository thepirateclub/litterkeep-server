//! Исходящий SMTP поверх TLS с явной проверкой локально заданного CA и SNI.
//!
//! Это standalone transport для route, где policy требует implicit TLS. Он не
//! разделяет plaintext pool: TLS identity является отдельным ключом ресурса.
use crate::outbound::{self, DeliveryOutcome};
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, StreamOwned,
    pki_types::{CertificateDer, ServerName},
};
use std::{
    io::{self, BufReader, Write},
    net::TcpStream,
    sync::Arc,
};

/// Одно implicit-TLS SMTP-соединение с закреплёнными CA и SNI context.
pub struct Connection {
    stream: BufReader<StreamOwned<ClientConnection, TcpStream>>,
    capabilities: outbound::EhloCapabilities,
    close_after_response: bool,
    timeouts: outbound::SmtpTimeouts,
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

    /// Выполняет одну SMTP transaction, не меняя TLS identity соединения.
    pub fn deliver(
        &mut self,
        sender: &str,
        recipient: &str,
        message: &[u8],
    ) -> io::Result<DeliveryOutcome> {
        self.deliver_recipients(sender, &[recipient], message)
    }

    /// Передаёт один DATA нескольким RCPT без размножения MIME payload.
    pub fn deliver_many(
        &mut self,
        sender: &str,
        recipients: &[String],
        message: &[u8],
    ) -> io::Result<DeliveryOutcome> {
        let recipients = recipients.iter().map(String::as_str).collect::<Vec<_>>();
        self.deliver_recipients(sender, &recipients, message)
    }

    fn deliver_recipients(
        &mut self,
        sender: &str,
        recipients: &[&str],
        message: &[u8],
    ) -> io::Result<DeliveryOutcome> {
        let binary_transfer = outbound::has_binary_transfer(message);
        let valid_message = match binary_transfer {
            true => outbound::validate_binary(message),
            false => outbound::validate_data(message).is_ok(),
        };
        if !valid_message {
            return Ok(DeliveryOutcome::PermanentFailure);
        }
        let envelope_requires_smtp_utf8 = crate::smtp::requires_smtp_utf8(sender)
            || recipients
                .iter()
                .any(|recipient| crate::smtp::requires_smtp_utf8(recipient));
        let mail_command = match outbound::build_mail_command(
            sender,
            envelope_requires_smtp_utf8,
            message,
            &self.capabilities,
        ) {
            outbound::MailCommand::Ready(command) => command,
            outbound::MailCommand::PermanentFailure => {
                return Ok(DeliveryOutcome::PermanentFailure);
            }
        };
        self.command(&mail_command)?;
        if let Some(outcome) = self.expect_command(outbound::ReplyExpectation::Mail)? {
            return Ok(outcome);
        }
        for recipient in recipients {
            self.command(&format!("RCPT TO:<{recipient}>\r\n"))?;
            if let Some(outcome) = self.expect_command(outbound::ReplyExpectation::Recipient)? {
                return Ok(outcome);
            }
        }
        if binary_transfer {
            self.stream
                .get_ref()
                .sock
                .set_write_timeout(Some(self.timeouts.data_block()))?;
            let timeouts = self.timeouts;
            let reply = outbound::bdat::send(
                &mut self.stream,
                message,
                |reader, command, payload, last| {
                    let phase = match last {
                        true => outbound::ReplyExpectation::BdatLast,
                        false => outbound::ReplyExpectation::Bdat,
                    };
                    reader
                        .get_ref()
                        .sock
                        .set_read_timeout(Some(timeouts.response(phase)))?;
                    let stream = reader.get_mut();
                    stream.write_all(command)?;
                    stream.write_all(payload)?;
                    stream.flush()
                },
            )?;
            return Ok(outbound::bdat::delivery_outcome(
                reply,
                &mut self.close_after_response,
            ));
        }
        self.command("DATA\r\n")?;
        if let Some(outcome) = self.expect_command(outbound::ReplyExpectation::Data)? {
            return Ok(outcome);
        }
        let stream = self.stream.get_mut();
        stream
            .sock
            .set_write_timeout(Some(self.timeouts.data_block()))?;
        outbound::write_data(stream, message)?;
        stream.flush()?;
        match self.expect_command(outbound::ReplyExpectation::Message)? {
            None => Ok(DeliveryOutcome::Delivered),
            Some(outcome) => Ok(outcome),
        }
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

    fn expect_command(
        &mut self,
        expectation: outbound::ReplyExpectation,
    ) -> io::Result<Option<DeliveryOutcome>> {
        self.stream
            .get_ref()
            .sock
            .set_read_timeout(Some(self.timeouts.response(expectation)))?;
        match outbound::expect(&mut self.stream, expectation)? {
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
            Some(DeliveryOutcome::Delivered) => Err(io::Error::other("unexpected SMTP outcome")),
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

fn command(
    reader: &mut BufReader<StreamOwned<ClientConnection, TcpStream>>,
    wire: &str,
) -> io::Result<()> {
    let stream = reader.get_mut();
    stream.write_all(wire.as_bytes())?;
    stream.flush()
}

fn expect_success(
    reader: &mut BufReader<StreamOwned<ClientConnection, TcpStream>>,
    expectation: outbound::ReplyExpectation,
) -> io::Result<()> {
    match outbound::expect(reader, expectation)? {
        outbound::ReplyResult::Accepted => Ok(()),
        outbound::ReplyResult::Rejected(DeliveryOutcome::Deferred) => {
            Err(io::Error::other("TLS SMTP peer deferred"))
        }
        outbound::ReplyResult::Rejected(DeliveryOutcome::PermanentFailure) => {
            Err(io::Error::other("TLS SMTP peer rejected"))
        }
        outbound::ReplyResult::Rejected(DeliveryOutcome::Delivered) => {
            Err(io::Error::other("unexpected SMTP outcome"))
        }
        outbound::ReplyResult::Closing => Err(io::Error::new(
            io::ErrorKind::ConnectionAborted,
            "TLS SMTP peer sent 421 and is closing the channel",
        )),
    }
}
