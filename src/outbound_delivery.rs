//! Одна envelope transaction plain SMTP поверх уже согласованного соединения.

use super::{
    Connection, DeliveryOutcome, EhloExtension, MailCommand, ReplyExpectation, build_mail_command,
    dsn_recipient_suffix, has_binary_transfer, validate_binary, validate_data, write_data,
};
use std::io::{self, Write};

impl Connection {
    /// Выполняет ровно одну SMTP envelope transaction на чистом канале.
    pub fn deliver(
        &mut self,
        sender: &str,
        recipient: &str,
        message: &[u8],
    ) -> io::Result<DeliveryOutcome> {
        self.deliver_with_dsn(sender, recipient, message, None)
    }

    /// Передаёт сохранённые DSN поля исходной SMTP transaction.
    pub(crate) fn deliver_with_dsn(
        &mut self,
        sender: &str,
        recipient: &str,
        message: &[u8],
        metadata: Option<&crate::queue::DsnDeliveryMetadata>,
    ) -> io::Result<DeliveryOutcome> {
        self.last_diagnostic = None;
        if metadata.is_some_and(|value| value.origin_recipients.len() > 1) {
            return Ok(DeliveryOutcome::Deferred);
        }
        let binary = has_binary_transfer(message);
        let valid = match binary {
            true => validate_binary(message),
            false => validate_data(message).is_ok(),
        };
        if !valid {
            return Ok(DeliveryOutcome::PermanentFailure);
        }
        let utf8 =
            crate::smtp::requires_smtp_utf8(sender) || crate::smtp::requires_smtp_utf8(recipient);
        let dsn = self.capabilities.supports(EhloExtension::Dsn);
        let suffix = match metadata.map(|value| value.origin_recipients.as_slice()) {
            Some([origin]) => match dsn_recipient_suffix(origin.as_ref(), dsn) {
                Some(suffix) => suffix,
                None => return Ok(DeliveryOutcome::PermanentFailure),
            },
            _ => String::new(),
        };
        let mail = match build_mail_command(
            sender,
            utf8,
            message,
            &self.capabilities,
            metadata.map(|value| &value.mail),
        ) {
            MailCommand::Ready(command) => command,
            MailCommand::PermanentFailure => return Ok(DeliveryOutcome::PermanentFailure),
        };
        self.command(&mail)?;
        if let Some(outcome) = self.expect_command(ReplyExpectation::Mail)? {
            return Ok(outcome);
        }
        self.command(&format!("RCPT TO:<{recipient}>{suffix}\r\n"))?;
        if let Some(outcome) = self.expect_command(ReplyExpectation::Recipient)? {
            return Ok(outcome);
        }
        let outcome = self.transfer_body(message, binary)?;
        match (outcome, dsn, relay_success_requested(metadata)) {
            (DeliveryOutcome::Delivered, false, true) => Ok(DeliveryOutcome::Relayed),
            (other, _, _) => Ok(other),
        }
    }

    /// Передаёт body через единственный допустимый DATA или BINARYMIME путь.
    fn transfer_body(&mut self, message: &[u8], binary: bool) -> io::Result<DeliveryOutcome> {
        if binary {
            return self.transfer_binary(message);
        }
        self.command("DATA\r\n")?;
        if let Some(outcome) = self.expect_command(ReplyExpectation::Data)? {
            return Ok(outcome);
        }
        self.writer
            .set_write_timeout(Some(self.timeouts.data_block()))?;
        write_data(&mut self.writer, message)?;
        self.writer.flush()?;
        match self.expect_command(ReplyExpectation::Message)? {
            Some(outcome) => Ok(outcome),
            None => Ok(DeliveryOutcome::Delivered),
        }
    }

    /// Передаёт binary MIME только CHUNKING-командой BDAT.
    fn transfer_binary(&mut self, message: &[u8]) -> io::Result<DeliveryOutcome> {
        self.writer
            .set_write_timeout(Some(self.timeouts.data_block()))?;
        let timeouts = self.timeouts;
        let reply = super::bdat::send(
            &mut self.reader,
            message,
            |reader, command, payload, last| {
                let phase = match last {
                    true => ReplyExpectation::BdatLast,
                    false => ReplyExpectation::Bdat,
                };
                reader
                    .get_ref()
                    .set_read_timeout(Some(timeouts.response(phase)))?;
                self.writer.write_all(command)?;
                self.writer.write_all(payload)?;
                self.writer.flush()
            },
        )?;
        Ok(super::bdat::delivery_outcome(
            reply,
            &mut self.close_after_response,
        ))
    }
}

/// Определяет явный запрос SUCCESS исходным единственным RCPT.
fn relay_success_requested(metadata: Option<&crate::queue::DsnDeliveryMetadata>) -> bool {
    crate::dsn::success_requested(metadata)
}
