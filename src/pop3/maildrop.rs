//! Неизменяемый POP3 maildrop snapshot и отложенные DELE-маркеры.

use crate::store::Message;
use std::collections::BTreeSet;

/// Snapshot общего INBOX принадлежит одной POP3 transaction-сессии.
pub struct Maildrop {
    messages: Vec<Message>,
    deleted: BTreeSet<u32>,
}

impl Maildrop {
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            deleted: BTreeSet::new(),
        }
    }
    pub fn stat(&self) -> (usize, usize) {
        self.visible().fold((0, 0), |(count, bytes), (_, message)| {
            (
                count.saturating_add(1),
                bytes.saturating_add(message.bytes.len()),
            )
        })
    }
    pub fn list(&self, number: Option<u32>) -> Result<Vec<(u32, usize)>, &'static str> {
        match number {
            Some(number) => self
                .message(number)
                .map(|message| vec![(number, message.bytes.len())]),
            None => Ok(self
                .visible()
                .map(|(number, message)| (number, message.bytes.len()))
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
    pub fn retrieve(&self, number: u32) -> Result<Vec<u8>, &'static str> {
        self.message(number)
            .map(|message| dot_stuff(&message.bytes))
    }
    pub fn top(&self, number: u32, body_lines: u32) -> Result<Vec<u8>, &'static str> {
        let message = self.message(number)?;
        Ok(dot_stuff(&top_bytes(&message.bytes, body_lines)))
    }
    pub fn delete(&mut self, number: u32) -> Result<(), &'static str> {
        self.message(number)?;
        self.deleted.insert(number);
        Ok(())
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
                let number = u32::try_from(offset.saturating_add(1)).ok()?;
                (!self.deleted.contains(&number)).then_some((number, message))
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
}

/// Долговечный UID mailbox entry, а не заголовок Message-ID, становится UIDL.
fn uidl(uid: u64) -> String {
    format!("lk-{uid:016x}")
}

/// Преобразует raw RFC822 octets в POP3 multiline wire, сохраняя dot-stuffing.
fn dot_stuff(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(bytes.len().saturating_add(8));
    let mut line = Vec::new();
    for byte in bytes {
        match byte {
            b'\n' => {
                append_line(&mut output, &line);
                line.clear();
            }
            b'\r' => {}
            value => line.push(*value),
        }
    }
    if !line.is_empty() || !bytes.ends_with(b"\n") {
        append_line(&mut output, &line);
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
        if line == b"\r\n" || line == b"\n" {
            break;
        }
    }
    for _ in 0..requested {
        let Some(line) = lines.next() else { break };
        result.extend_from_slice(line);
    }
    result
}
