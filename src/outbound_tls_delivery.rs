//! Envelope и передача тела SMTP через TLS session.

use super::{Connection, outbound};
use crate::outbound::DeliveryOutcome;
use std::io::{self, Write};

impl Connection {
    /// Выполняет одну SMTP transaction, не меняя TLS identity соединения.
    pub fn deliver(
        &mut self,
        sender: &str,
        recipient: &str,
        message: &[u8],
    ) -> io::Result<DeliveryOutcome> {
        self.deliver_with_dsn(sender, recipient, message, None)
    }

    /// Переносит сохранённые RFC 3461 параметры forwarding job-а.
    pub(crate) fn deliver_with_dsn(
        &mut self,
        sender: &str,
        recipient: &str,
        message: &[u8],
        metadata: Option<&crate::queue::DsnDeliveryMetadata>,
    ) -> io::Result<DeliveryOutcome> {
        self.last_diagnostic = None;
        self.deliver_recipients(sender, &[recipient], message, metadata)
    }

    /// Передаёт один DATA нескольким RCPT без размножения MIME payload.
    pub fn deliver_many(
        &mut self,
        sender: &str,
        recipients: &[String],
        message: &[u8],
    ) -> io::Result<DeliveryOutcome> {
        let recipients = recipients.iter().map(String::as_str).collect::<Vec<_>>();
        self.deliver_recipients(sender, &recipients, message, None)
    }

    /// Проверяет envelope policy до записи SMTP-команд.
    fn deliver_recipients(
        &mut self,
        sender: &str,
        recipients: &[&str],
        message: &[u8],
        metadata: Option<&crate::queue::DsnDeliveryMetadata>,
    ) -> io::Result<DeliveryOutcome> {
        let binary = outbound::has_binary_transfer(message);
        if metadata.is_some_and(|value| value.origin_recipients.len() > 1) {
            return Ok(DeliveryOutcome::Deferred);
        }
        let valid = match binary {
            true => outbound::validate_binary(message),
            false => outbound::validate_data(message).is_ok(),
        };
        if !valid {
            return Ok(DeliveryOutcome::PermanentFailure);
        }
        let utf8 = crate::smtp::requires_smtp_utf8(sender)
            || recipients
                .iter()
                .any(|recipient| crate::smtp::requires_smtp_utf8(recipient));
        let dsn = self.capabilities.supports(outbound::EhloExtension::Dsn);
        let recipient_suffix = match metadata.map(|value| value.origin_recipients.as_slice()) {
            Some([origin]) => match outbound::dsn_recipient_suffix(origin.as_ref(), dsn) {
                Some(suffix) => suffix,
                None => return Ok(DeliveryOutcome::PermanentFailure),
            },
            _ => String::new(),
        };
        let mail = match outbound::build_mail_command(
            sender,
            utf8,
            message,
            &self.capabilities,
            metadata.map(|value| &value.mail),
        ) {
            outbound::MailCommand::Ready(command) => command,
            outbound::MailCommand::PermanentFailure => {
                return Ok(DeliveryOutcome::PermanentFailure);
            }
        };
        self.command(&mail)?;
        if let Some(outcome) = self.expect_command(outbound::ReplyExpectation::Mail)? {
            return Ok(outcome);
        }
        if let Some(outcome) = self.send_recipients(recipients, &recipient_suffix)? {
            return Ok(outcome);
        }
        self.send_body(message, binary)
    }

    /// Сериализует recipient-scoped DSN для каждого разрешённого RCPT.
    fn send_recipients(
        &mut self,
        recipients: &[&str],
        suffix: &str,
    ) -> io::Result<Option<DeliveryOutcome>> {
        for recipient in recipients {
            self.command(&format!("RCPT TO:<{recipient}>{suffix}\r\n"))?;
            if let Some(outcome) = self.expect_command(outbound::ReplyExpectation::Recipient)? {
                return Ok(Some(outcome));
            }
        }
        Ok(None)
    }

    /// Выбирает DATA либо BDAT в соответствии с содержимым сообщения.
    fn send_body(&mut self, message: &[u8], binary: bool) -> io::Result<DeliveryOutcome> {
        if binary {
            return self.send_binary(message);
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

    /// Посылает binary message только при согласованных BINARYMIME/CHUNKING.
    fn send_binary(&mut self, message: &[u8]) -> io::Result<DeliveryOutcome> {
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
        Ok(outbound::bdat::delivery_outcome(
            reply,
            &mut self.close_after_response,
        ))
    }
}
