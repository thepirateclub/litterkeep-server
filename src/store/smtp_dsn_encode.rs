//! Проверка и кодирование RFC 3461 envelope sidecar frame.

use super::format::{self, MAX_FRAME, MAX_RECIPIENTS};
use crate::smtp::{
    DsnEnvelope, NotifyCondition, NotifyPolicy, OriginalAddressType, OriginalRecipient, RcptTo,
    ReturnContent,
};
use std::collections::HashSet;
use std::io;

/// Кодирует bounded envelope payload с checksum и commit marker-ом.
pub(super) fn frame(transaction_id: u64, envelope: &DsnEnvelope) -> io::Result<Vec<u8>> {
    validate_envelope(envelope)?;
    let mut payload = Writer::default();
    payload.string(&envelope.reverse_path)?;
    payload.byte(return_tag(envelope.mail.return_content));
    payload.optional_bytes(envelope.mail.envelope_id.as_deref())?;
    payload.count(envelope.recipients.len())?;
    for recipient in &envelope.recipients {
        encode_recipient(&mut payload, recipient)?;
    }
    let payload = payload.bytes;
    let frame_length = 18_usize
        .checked_add(payload.len())
        .and_then(|length| length.checked_add(12))
        .ok_or_else(|| invalid("SMTP DSN frame length overflow"))?;
    if frame_length > MAX_FRAME {
        return Err(invalid("SMTP DSN frame limit"));
    }
    let mut bytes = Vec::with_capacity(frame_length);
    bytes.extend_from_slice(&format::MAGIC);
    bytes.extend_from_slice(&format::VERSION.to_be_bytes());
    bytes.extend_from_slice(&transaction_id.to_be_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&payload);
    let sum = format::checksum(&bytes);
    bytes.extend_from_slice(&sum.to_be_bytes());
    bytes.extend_from_slice(&format::COMMIT.to_be_bytes());
    Ok(bytes)
}

/// Проверяет envelope limits до выделения durable payload.
fn validate_envelope(envelope: &DsnEnvelope) -> io::Result<()> {
    if envelope.recipients.is_empty() || envelope.recipients.len() > MAX_RECIPIENTS {
        return Err(invalid("SMTP DSN recipient count"));
    }
    if envelope.reverse_path.len() > 256 {
        return Err(invalid("SMTP DSN reverse-path limit"));
    }
    if envelope
        .mail
        .envelope_id
        .as_ref()
        .is_some_and(|value| value.len() > 100)
    {
        return Err(invalid("SMTP DSN envelope-id limit"));
    }
    for recipient in &envelope.recipients {
        if recipient.address.is_empty() || recipient.address.len() > 256 {
            return Err(invalid("SMTP DSN forward-path limit"));
        }
        if let Some(original) = &recipient.dsn.original_recipient
            && !valid_original_recipient(original)
        {
            return Err(invalid("SMTP DSN original-recipient limit"));
        }
        if let Some(NotifyPolicy::Conditions(conditions)) = &recipient.dsn.notify
            && conditions.is_empty()
        {
            return Err(invalid("SMTP DSN NOTIFY list is empty"));
        }
    }
    Ok(())
}

fn valid_original_recipient(value: &OriginalRecipient) -> bool {
    if value.address.is_empty() || value.address.len() > 500 {
        return false;
    }
    match value.address_type {
        OriginalAddressType::Rfc822 => value
            .address
            .iter()
            .all(|byte| (b' '..=b'~').contains(byte)),
        OriginalAddressType::Utf8 => std::str::from_utf8(&value.address)
            .ok()
            .is_some_and(crate::smtp::is_utf8_mailbox),
    }
}

/// Записывает recipient-specific NOTIFY и ORCPT рядом с forward-path.
fn encode_recipient(writer: &mut Writer, recipient: &RcptTo) -> io::Result<()> {
    writer.string(&recipient.address)?;
    writer.notify(recipient.dsn.notify.as_ref());
    match &recipient.dsn.original_recipient {
        Some(original) => {
            writer.byte(2);
            writer.byte(address_type_tag(original.address_type));
            writer.bytes(&original.address)?;
        }
        None => writer.byte(0),
    }
    Ok(())
}

fn return_tag(value: Option<ReturnContent>) -> u8 {
    match value {
        None => 0,
        Some(ReturnContent::Headers) => 1,
        Some(ReturnContent::Full) => 2,
    }
}

fn address_type_tag(value: OriginalAddressType) -> u8 {
    match value {
        OriginalAddressType::Rfc822 => 1,
        OriginalAddressType::Utf8 => 2,
    }
}

/// Payload builder ограничивает переменные длины до записи поля.
#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn byte(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn string(&mut self, value: &str) -> io::Result<()> {
        self.bytes(value.as_bytes())
    }

    fn bytes(&mut self, value: &[u8]) -> io::Result<()> {
        let length = u16::try_from(value.len()).map_err(|_| invalid("SMTP DSN field limit"))?;
        self.bytes.extend_from_slice(&length.to_be_bytes());
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn optional_bytes(&mut self, value: Option<&[u8]>) -> io::Result<()> {
        match value {
            Some(value) => {
                self.byte(1);
                self.bytes(value)?;
            }
            None => self.byte(0),
        }
        Ok(())
    }

    fn count(&mut self, value: usize) -> io::Result<()> {
        if value > MAX_RECIPIENTS {
            return Err(invalid("SMTP DSN recipient count"));
        }
        let count = u16::try_from(value).map_err(|_| invalid("SMTP DSN recipient count"))?;
        self.bytes.extend_from_slice(&count.to_be_bytes());
        Ok(())
    }

    fn notify(&mut self, policy: Option<&NotifyPolicy>) {
        match policy {
            None => self.byte(0),
            Some(NotifyPolicy::Never) => self.byte(1),
            Some(NotifyPolicy::Conditions(conditions)) => {
                self.byte(2);
                self.byte(condition_mask(conditions));
            }
        }
    }
}

fn condition_mask(conditions: &HashSet<NotifyCondition>) -> u8 {
    conditions.iter().fold(0, |mask, condition| {
        mask | match condition {
            NotifyCondition::Success => 1,
            NotifyCondition::Failure => 2,
            NotifyCondition::Delay => 4,
        }
    })
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
