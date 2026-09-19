//! Инкрементальное владение синхронизирующим IMAP literal.

/// Полученный полностью literal вместе с командным контекстом владельца.
pub(super) struct CompletedLiteral {
    pub(super) tag: String,
    pub(super) mailbox: String,
    pub(super) bytes: Vec<u8>,
    pub(super) metadata: crate::store::AppendMetadata,
}

/// Результат одного шага приёма без блокировки и ожидания в цикле.
pub(super) enum LiteralProgress {
    Waiting,
    Complete(CompletedLiteral),
    InvalidTerminator(String),
}

/// Буфер ровно одного literal, заранее ограниченного admission policy.
pub(super) struct LiteralReceiver {
    tag: String,
    mailbox: String,
    expected: usize,
    bytes: Vec<u8>,
    metadata: crate::store::AppendMetadata,
}

impl LiteralReceiver {
    pub(super) fn new(
        tag: String,
        mailbox: String,
        expected: usize,
        metadata: crate::store::AppendMetadata,
    ) -> Self {
        Self {
            tag,
            mailbox,
            expected,
            bytes: Vec::with_capacity(expected),
            metadata,
        }
    }

    /// Поглощает доступный фрагмент и сохраняет остаток pipelined input.
    pub(super) fn consume(&mut self, input: &mut Vec<u8>) -> LiteralProgress {
        let remaining = self.expected.saturating_sub(self.bytes.len());
        let consumed = remaining.min(input.len());
        self.bytes.extend(input.drain(..consumed));
        if self.bytes.len() < self.expected || input.len() < 2 {
            return LiteralProgress::Waiting;
        }
        match input.get(..2) {
            Some(b"\r\n") => {
                input.drain(..2);
                LiteralProgress::Complete(CompletedLiteral {
                    tag: std::mem::take(&mut self.tag),
                    mailbox: std::mem::take(&mut self.mailbox),
                    bytes: std::mem::take(&mut self.bytes),
                    metadata: self.metadata,
                })
            }
            Some(_) | None => LiteralProgress::InvalidTerminator(std::mem::take(&mut self.tag)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Каждая возможная граница двух TCP-фрагментов даёт те же байты literal.
    #[test]
    fn accepts_every_two_chunk_split() {
        let wire = b"a\r\nbc\r\nNEXT";
        for split in 0..7 {
            let mut receiver = LiteralReceiver::new(
                "t".into(),
                "bob".into(),
                5,
                crate::store::AppendMetadata::default(),
            );
            let mut input = wire.get(..split).expect("prefix").to_vec();
            assert!(matches!(
                receiver.consume(&mut input),
                LiteralProgress::Waiting
            ));
            input.extend_from_slice(wire.get(split..).expect("suffix"));
            let LiteralProgress::Complete(done) = receiver.consume(&mut input) else {
                panic!("split {split} did not complete");
            };
            assert_eq!(done.bytes, b"a\r\nbc");
            assert_eq!(input, b"NEXT");
        }
    }
}
