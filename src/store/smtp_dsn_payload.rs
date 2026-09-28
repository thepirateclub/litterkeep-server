//! Декодирование typed SMTP envelope из payload журнала DSN.

use super::cursor::{self, Cursor, FrameError};
use crate::smtp::{
    DsnEnvelope, MailDsn, NotifyCondition, NotifyPolicy, OriginalAddressType, OriginalRecipient,
    RcptTo, RecipientDsn, ReturnContent,
};
use std::collections::HashSet;

const MAX_ADDRESS: usize = 256;
const MAX_ENVID: usize = 100;
const MAX_ORCPT: usize = 500;

/// Восстанавливает envelope и проверяет каждое ограниченное поле.
pub(super) fn decode(bytes: &[u8], version: u16) -> Result<DsnEnvelope, FrameError> {
    let mut reader = Reader::new(bytes);
    let reverse_path = reader.text(MAX_ADDRESS)?;
    let mail = reader.mail_dsn()?;
    let count = usize::from(u16::from_be_bytes(reader.cursor.take::<2>()?));
    if count > super::format::MAX_RECIPIENTS {
        return Err(reader.invalid("SMTP DSN recipient count limit"));
    }
    let mut recipients = Vec::with_capacity(count);
    for _ in 0..count {
        recipients.push(reader.recipient(version)?);
    }
    if reader.cursor.position() != bytes.len() {
        return Err(reader.invalid("trailing SMTP DSN payload"));
    }
    Ok(DsnEnvelope {
        reverse_path,
        mail,
        recipients,
    })
}

struct Reader<'a> {
    cursor: Cursor<'a>,
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            cursor: Cursor::new(bytes, 0),
            bytes,
        }
    }

    fn byte(&mut self) -> Result<u8, FrameError> {
        self.cursor
            .take::<1>()
            .map(|byte| byte.into_iter().next().unwrap_or_default())
    }

    fn text(&mut self, limit: usize) -> Result<String, FrameError> {
        let length = usize::from(u16::from_be_bytes(self.cursor.take::<2>()?));
        if length > limit {
            return Err(self.invalid("SMTP DSN text length limit"));
        }
        let start = self.cursor.position();
        self.cursor.take_slice(length)?;
        let value = self
            .bytes
            .get(start..start.saturating_add(length))
            .ok_or(FrameError::Incomplete)?;
        let text =
            std::str::from_utf8(value).map_err(|_| self.invalid("SMTP DSN text is not UTF-8"))?;
        Ok(text.to_owned())
    }

    fn mail_dsn(&mut self) -> Result<MailDsn, FrameError> {
        let return_content = match self.byte()? {
            0 => None,
            1 => Some(ReturnContent::Headers),
            2 => Some(ReturnContent::Full),
            _ => return Err(self.invalid("SMTP DSN RET tag")),
        };
        let envelope_id = match self.byte()? {
            0 => None,
            1 => {
                let id = self.bytes(MAX_ENVID)?;
                if id.is_empty() {
                    return Err(self.invalid("empty SMTP DSN ENVID"));
                }
                Some(id)
            }
            _ => return Err(self.invalid("SMTP DSN ENVID tag")),
        };
        Ok(MailDsn {
            return_content,
            envelope_id,
        })
    }

    fn recipient(&mut self, version: u16) -> Result<RcptTo, FrameError> {
        let address = self.text(MAX_ADDRESS)?;
        let notify = self.notify()?;
        let original_recipient = match self.byte()? {
            0 => None,
            2 => {
                let address_type = match self.byte()? {
                    1 => OriginalAddressType::Rfc822,
                    2 if version >= super::format::VERSION => OriginalAddressType::Utf8,
                    _ => return Err(self.invalid("unsupported SMTP DSN ORCPT type")),
                };
                let value = self.bytes(MAX_ORCPT)?;
                if value.is_empty() {
                    return Err(self.invalid("empty SMTP DSN ORCPT"));
                }
                let valid = match address_type {
                    OriginalAddressType::Rfc822 => {
                        value.iter().all(|byte| (b' '..=b'~').contains(byte))
                    }
                    OriginalAddressType::Utf8 => std::str::from_utf8(&value)
                        .ok()
                        .is_some_and(crate::smtp::is_utf8_mailbox),
                };
                if !valid {
                    return Err(self.invalid("invalid SMTP DSN ORCPT bytes"));
                }
                Some(OriginalRecipient {
                    address_type,
                    address: value,
                })
            }
            _ => return Err(self.invalid("SMTP DSN ORCPT tag")),
        };
        Ok(RcptTo {
            address,
            dsn: RecipientDsn {
                notify,
                original_recipient,
            },
        })
    }

    fn notify(&mut self) -> Result<Option<NotifyPolicy>, FrameError> {
        match self.byte()? {
            0 => Ok(None),
            2 => {
                let mask = self.byte()?;
                if mask == 0x10 {
                    return Ok(Some(NotifyPolicy::Never));
                }
                let mut conditions = HashSet::new();
                for (bit, condition) in [
                    (1, NotifyCondition::Success),
                    (2, NotifyCondition::Failure),
                    (4, NotifyCondition::Delay),
                ] {
                    if mask & bit != 0 {
                        conditions.insert(condition);
                    }
                }
                if mask == 0 || mask & !0x07 != 0 {
                    return Err(self.invalid("SMTP DSN NOTIFY mask"));
                }
                Ok(Some(NotifyPolicy::Conditions(conditions)))
            }
            1 => Ok(Some(NotifyPolicy::Never)),
            _ => Err(self.invalid("SMTP DSN NOTIFY tag")),
        }
    }

    fn bytes(&mut self, limit: usize) -> Result<Vec<u8>, FrameError> {
        let length = usize::from(u16::from_be_bytes(self.cursor.take::<2>()?));
        if length > limit {
            return Err(self.invalid("SMTP DSN byte length limit"));
        }
        let value = self.cursor.take_slice(length)?;
        Ok(value.to_vec())
    }

    fn invalid(&self, message: &'static str) -> FrameError {
        FrameError::Invalid(cursor::invalid(message))
    }
}
