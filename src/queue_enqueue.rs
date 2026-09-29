//! Запись owned delivery и его переходов в durable очередь.

use super::{
    OwnedDelivery, Queue, State, encode_dsn_notice, encode_sender, queue_owned, validate_token,
};
use std::{
    fs::OpenOptions,
    io::{self, Write},
};

impl Queue {
    /// Пишет job до global Store commit. Видимым его делает только committed ID.
    pub fn enqueue_owned(
        &self,
        transaction_id: u64,
        sender: &str,
        recipient: &str,
        original_recipient_index: Option<usize>,
    ) -> io::Result<()> {
        self.enqueue_owned_many(
            transaction_id,
            sender,
            recipient,
            &original_recipient_index.into_iter().collect::<Vec<_>>(),
        )
    }

    /// Сохраняет один transport route со всеми исходными RCPT ordinal-ами.
    pub fn enqueue_owned_many(
        &self,
        transaction_id: u64,
        sender: &str,
        recipient: &str,
        original_recipient_indices: &[usize],
    ) -> io::Result<()> {
        queue_owned::validate_recipient_indices(original_recipient_indices)?;
        validate_token(sender)?;
        validate_token(recipient)?;
        let sender_token = encode_sender(sender);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(
            file,
            "{} {transaction_id} 0 0 {sender_token} {recipient} {} - -",
            State::Pending.wire(),
            queue_owned::encode_recipient_indices(original_recipient_indices)
        )?;
        file.sync_all()
    }

    /// Фиксирует состояние owned job без изменения immutable envelope данных.
    pub fn transition_owned(&self, delivery: &OwnedDelivery, state: State) -> io::Result<()> {
        queue_owned::validate_recipient_indices(&delivery.original_recipient_indices)?;
        validate_token(&delivery.sender)?;
        validate_token(&delivery.recipient)?;
        let sender_token = encode_sender(&delivery.sender);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(
            file,
            "{} {} {} {} {} {} {} {} {}",
            state.wire(),
            delivery.transaction_id,
            delivery.attempts,
            delivery.next_attempt_epoch,
            sender_token,
            delivery.recipient,
            queue_owned::encode_recipient_indices(&delivery.original_recipient_indices),
            encode_dsn_notice(delivery.dsn_notice),
            delivery
                .dsn_diagnostic
                .as_ref()
                .map_or_else(|| "-".to_owned(), |value| value.journal_token())
        )?;
        file.sync_all()
    }
}
