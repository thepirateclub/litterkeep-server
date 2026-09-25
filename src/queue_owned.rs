//! Recovery и due-отбор owned outbound jobs из append-only queue journal.
use super::{OwnedDelivery, Queue, State, invalid};
use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
};

impl Queue {
    /// Возвращает IDs transaction, для которых журнал содержит owned SMTP job.
    pub fn owned_transaction_ids(&self) -> io::Result<HashSet<u64>> {
        let source = self.source()?;
        let mut result = HashSet::new();
        for line in source.lines() {
            if let Some(delivery) = parse_owned(line)? {
                result.insert(delivery.transaction_id);
            }
        }
        Ok(result)
    }

    /// Возвращает committed jobs, включая deferred jobs с будущим deadline.
    pub fn pending_owned(&self, committed: &HashSet<u64>) -> io::Result<Vec<OwnedDelivery>> {
        Ok(self
            .owned_latest()?
            .into_values()
            .filter(|delivery| visible(delivery, committed))
            .collect())
    }

    /// Возвращает только jobs, чей durable deadline уже наступил.
    pub fn due_owned(
        &self,
        committed: &HashSet<u64>,
        now_epoch: u64,
    ) -> io::Result<Vec<OwnedDelivery>> {
        Ok(self
            .owned_latest()?
            .into_values()
            .filter(|delivery| visible(delivery, committed))
            .filter(|delivery| delivery.next_attempt_epoch <= now_epoch)
            .collect())
    }

    fn source(&self) -> io::Result<String> {
        match fs::read_to_string(&self.path) {
            Ok(value) => Ok(value),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
            Err(error) => Err(error),
        }
    }

    fn owned_latest(&self) -> io::Result<BTreeMap<(u64, String), OwnedDelivery>> {
        let mut latest = BTreeMap::new();
        for line in self.source()?.lines() {
            if let Some(delivery) = parse_owned(line)? {
                latest.insert(
                    (delivery.transaction_id, delivery.recipient.clone()),
                    delivery,
                );
            }
        }
        Ok(latest)
    }
}

fn parse_owned(line: &str) -> io::Result<Option<OwnedDelivery>> {
    let mut fields = line.split_whitespace();
    let Some(state) = fields.next() else {
        return Ok(None);
    };
    let Some(transaction) = fields.next() else {
        return Ok(None);
    };
    let Some(candidate) = fields.next() else {
        return Ok(None);
    };
    let Some(fourth) = fields.next() else {
        return Ok(None);
    };
    let transaction_id = transaction
        .parse()
        .map_err(|_| invalid("bad owned transaction"))?;
    let state = State::parse(state)?;
    let (attempts, next_attempt_epoch, sender, recipient) = match candidate.parse::<u8>() {
        Ok(attempts) => parse_versioned(attempts, fourth, &mut fields)?,
        Err(_) => parse_legacy(candidate, fourth, &mut fields)?,
    };
    let (state, next_attempt_epoch) = match state {
        State::InFlight => (State::Deferred, 0),
        state => (state, next_attempt_epoch),
    };
    Ok(Some(OwnedDelivery {
        transaction_id,
        attempts,
        next_attempt_epoch,
        sender: decode_sender(sender).into(),
        recipient: recipient.into(),
        state,
    }))
}

/// Восстанавливает пустой SMTP reverse-path из его queue wire token-а.
fn decode_sender(value: &str) -> &str {
    match value {
        "<>" => "",
        address => address,
    }
}

fn parse_versioned<'a>(
    attempts: u8,
    fourth: &'a str,
    fields: &mut std::str::SplitWhitespace<'a>,
) -> io::Result<(u8, u64, &'a str, &'a str)> {
    match fourth.parse::<u64>() {
        Ok(next_attempt_epoch) => match (fields.next(), fields.next(), fields.next()) {
            (Some(sender), Some(recipient), None) => {
                Ok((attempts, next_attempt_epoch, sender, recipient))
            }
            _ => Err(invalid("bad current owned queue record")),
        },
        Err(_) => match (fields.next(), fields.next()) {
            (Some(recipient), None) => Ok((attempts, 0, fourth, recipient)),
            _ => Err(invalid("bad previous owned queue record")),
        },
    }
}

fn parse_legacy<'a>(
    sender: &'a str,
    recipient: &'a str,
    fields: &mut std::str::SplitWhitespace<'a>,
) -> io::Result<(u8, u64, &'a str, &'a str)> {
    match fields.next() {
        None => Ok((0, 0, sender, recipient)),
        Some(_) => Err(invalid("bad legacy owned queue record")),
    }
}

fn visible(delivery: &OwnedDelivery, committed: &HashSet<u64>) -> bool {
    match (
        committed.contains(&delivery.transaction_id),
        matches!(delivery.state, State::Pending | State::Deferred),
    ) {
        (true, true) => true,
        (true, false) | (false, _) => false,
    }
}
