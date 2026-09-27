//! Неизменяемый POP3 maildrop snapshot и отложенные DELE-маркеры.

use super::message_profile::{MessageEncoding, MessageProfile, MimeTransferPolicy};
use crate::store::Message;
use std::collections::{BTreeMap, BTreeSet};

/// Snapshot общего INBOX принадлежит одной POP3 transaction-сессии.
pub struct Maildrop {
    messages: Vec<Message>,
    deleted: BTreeSet<u32>,
    /// Кэш UID позволяет не сканировать байты повторно на каждом LIST/RETR.
    message_profiles: BTreeMap<u64, MessageProfile>,
}

/// Типизированные причины отказа DELE, соответствующие номеру maildrop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeleteError {
    /// Сообщение с таким номером отсутствует в snapshot.
    NoSuchMessage,
    /// RFC 1939 запрещает повторно ссылаться на уже удалённое сообщение.
    AlreadyDeleted,
}

impl Maildrop {
    pub fn new(messages: Vec<Message>) -> Self {
        let message_profiles = messages
            .iter()
            .map(|message| (message.uid, MessageProfile::new(&message.bytes)))
            .collect();
        Self {
            messages,
            deleted: BTreeSet::new(),
            message_profiles,
        }
    }
    pub fn stat(&self) -> (usize, usize) {
        self.visible().fold((0, 0), |(count, bytes), (_, message)| {
            (
                count.saturating_add(1),
                bytes.saturating_add(self.profile(message).octets),
            )
        })
    }
    pub fn list(&self, number: Option<u32>) -> Result<Vec<(u32, usize)>, &'static str> {
        match number {
            Some(number) => self
                .message(number)
                .map(|message| vec![(number, self.profile(message).octets)]),
            None => Ok(self
                .visible()
                .map(|(number, message)| (number, self.profile(message).octets))
                .collect()),
        }
    }
    pub fn uidl(&self, number: Option<u32>) -> Result<Vec<(u32, String)>, &'static str> {
        match number {
            Some(number) => self
                .message(number)
                .map(|message| vec![(number, uidl(message.uid))]),
            None => Ok(self
                .visible()
                .map(|(number, message)| (number, uidl(message.uid)))
                .collect()),
        }
    }
    /// Проверяет, содержит ли выбранное письмо корректные UTF-8 символы вне ASCII.
    /// Полный snapshot сканируется один раз при создании, затем lookup идёт по UID.
    pub(crate) fn encoding(&self, number: Option<u32>) -> Result<MessageEncoding, &'static str> {
        match number {
            Some(number) => self
                .message(number)
                .map(|message| self.profile(message).encoding),
            None => Ok(self
                .visible()
                .fold(MessageEncoding::Ascii, |encoding, (_, message)| {
                    encoding.combine(self.profile(message).encoding)
                })),
        }
    }
    /// Запрещает TOP/RETR для MIME binary CTE по RFC 6856 даже в UTF8 mode.
    pub(crate) fn transfer_policy(&self, number: u32) -> Result<MimeTransferPolicy, &'static str> {
        self.message(number)
            .map(|message| self.profile(message).transfer_policy)
    }
    pub fn retrieve(&self, number: u32) -> Result<Vec<u8>, &'static str> {
        self.message(number)
            .map(|message| dot_stuff(&message.bytes))
    }
    pub fn top(&self, number: u32, body_lines: u32) -> Result<Vec<u8>, &'static str> {
        let message = self.message(number)?;
        Ok(dot_stuff(&top_bytes(&message.bytes, body_lines)))
    }
    pub fn delete(&mut self, number: u32) -> Result<(), DeleteError> {
        match self.entry(number) {
            None => Err(DeleteError::NoSuchMessage),
            Some(_) => match self.deleted.insert(number) {
                true => Ok(()),
                false => Err(DeleteError::AlreadyDeleted),
            },
        }
    }
    pub fn reset(&mut self) {
        self.deleted.clear();
    }
    pub fn deleted_uids(&self) -> Vec<u64> {
        self.deleted
            .iter()
            .filter_map(|number| self.entry(*number).map(|message| message.uid))
            .collect()
    }
    fn visible(&self) -> impl Iterator<Item = (u32, &Message)> {
        self.messages
            .iter()
            .enumerate()
            .filter_map(|(offset, message)| {
                // Номер задаётся при открытии maildrop и остаётся стабильным,
                // даже когда DELE временно скрывает запись из ответов.
                let number = u32::try_from(offset.saturating_add(1)).ok()?;
                match self.deleted.contains(&number) {
                    true => None,
                    false => Some((number, message)),
                }
            })
    }
    fn message(&self, number: u32) -> Result<&Message, &'static str> {
        match self.deleted.contains(&number) {
            true => Err("message deleted"),
            false => self.entry(number).ok_or("no such message"),
        }
    }
    fn entry(&self, number: u32) -> Option<&Message> {
        usize::try_from(number.checked_sub(1)?)
            .ok()
            .and_then(|offset| self.messages.get(offset))
    }

    /// Читает профиль, построенный одновременно со snapshot maildrop.
    fn profile(&self, message: &Message) -> MessageProfile {
        self.message_profiles
            .get(&message.uid)
            .copied()
            .unwrap_or_else(|| MessageProfile {
                encoding: MessageProfile::classify(&message.bytes),
                octets: MessageProfile::reported_size(&message.bytes),
                transfer_policy: MimeTransferPolicy::Unverifiable,
            })
    }
}

/// Долговечный UID mailbox entry, а не заголовок Message-ID, становится UIDL.
fn uidl(uid: u64) -> String {
    format!("lk-{uid:016x}")
}

/// Преобразует raw RFC822 octets в POP3 multiline wire, сохраняя dot-stuffing.
/// Нормализует пары CRLF и LF, но не вырезает CR из содержимого строки.
fn dot_stuff(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(bytes.len().saturating_add(8));
    for source_line in bytes.split_inclusive(|byte| *byte == b'\n') {
        match source_line.strip_suffix(b"\n") {
            Some(without_lf) => {
                let line = without_lf.strip_suffix(b"\r").unwrap_or(without_lf);
                append_line(&mut output, line);
            }
            None => append_line(&mut output, source_line),
        }
    }
    output.extend_from_slice(b".\r\n");
    output
}
fn append_line(output: &mut Vec<u8>, line: &[u8]) {
    if line.first() == Some(&b'.') {
        output.push(b'.');
    }
    output.extend_from_slice(line);
    output.extend_from_slice(b"\r\n");
}
fn top_bytes(bytes: &[u8], requested: u32) -> Vec<u8> {
    let mut result = Vec::new();
    let mut lines = bytes.split_inclusive(|byte| *byte == b'\n');
    for line in lines.by_ref() {
        result.extend_from_slice(line);
        if matches!(line, b"\r\n" | b"\n") {
            break;
        }
    }
    for _ in 0..requested {
        let Some(line) = lines.next() else { break };
        result.extend_from_slice(line);
    }
    result
}
