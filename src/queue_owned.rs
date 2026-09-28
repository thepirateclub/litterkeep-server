//! Recovery и due-отбор owned outbound jobs из append-only queue journal.
use super::{OwnedDelivery, Queue, State, invalid};
use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
};

type OwnedLatest = BTreeMap<(u64, String, Option<usize>), OwnedDelivery>;
type ParsedOwned<'a> = (
    u8,
    u64,
    &'a str,
    &'a str,
    Option<usize>,
    Option<crate::dsn::Failure>,
);

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
            .filter(|delivery| {
                delivery.dsn_failure.is_some() || delivery.next_attempt_epoch <= now_epoch
            })
            .collect())
    }

    fn source(&self) -> io::Result<String> {
        match fs::read_to_string(&self.path) {
            Ok(value) => Ok(value),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
            Err(error) => Err(error),
        }
    }

    fn owned_latest(&self) -> io::Result<OwnedLatest> {
        let mut latest = BTreeMap::new();
        for line in self.source()?.lines() {
            if let Some(delivery) = parse_owned(line)? {
                latest.insert(
                    (
                        delivery.transaction_id,
                        delivery.recipient.clone(),
                        delivery.original_recipient_index,
                    ),
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
    let (attempts, next_attempt_epoch, sender, recipient, original_recipient_index, dsn_failure) =
        match candidate.parse::<u8>() {
            Ok(attempts) => parse_versioned(attempts, fourth, &mut fields)?,
            Err(_) => {
                let (attempts, epoch, sender, recipient) =
                    parse_legacy(candidate, fourth, &mut fields)?;
                (attempts, epoch, sender, recipient, None, None)
            }
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
        original_recipient_index,
        dsn_metadata: None,
        dsn_failure,
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
) -> io::Result<ParsedOwned<'a>> {
    match fourth.parse::<u64>() {
        Ok(next_attempt_epoch) => parse_current_fields(attempts, next_attempt_epoch, fields),
        Err(_) => match (fields.next(), fields.next()) {
            (Some(recipient), None) => Ok((attempts, 0, fourth, recipient, None, None)),
            _ => Err(invalid("bad previous owned queue record")),
        },
    }
}

/// Читает текущий queue frame и совместимый вариант до появления RCPT индекса.
fn parse_current_fields<'a>(
    attempts: u8,
    epoch: u64,
    fields: &mut std::str::SplitWhitespace<'a>,
) -> io::Result<ParsedOwned<'a>> {
    let (Some(sender), Some(recipient)) = (fields.next(), fields.next()) else {
        return Err(invalid("bad current owned queue record"));
    };
    let original_recipient_index = match fields.next() {
        None => None,
        Some("-") => None,
        Some(value) => Some(
            usize::try_from(
                value
                    .parse::<u64>()
                    .map_err(|_| invalid("bad RCPT index"))?,
            )
            .map_err(|_| invalid("RCPT index overflow"))?,
        ),
    };
    let dsn_failure = match fields.next() {
        None | Some("-") => None,
        Some(value) => Some(
            crate::dsn::Failure::from_journal_tag(value)
                .ok_or_else(|| invalid("bad owned DSN notice"))?,
        ),
    };
    match fields.next() {
        None => Ok((
            attempts,
            epoch,
            sender,
            recipient,
            original_recipient_index,
            dsn_failure,
        )),
        Some(_) => Err(invalid("bad current owned queue record")),
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
        matches!(delivery.state, State::Pending | State::Deferred)
            || delivery.dsn_failure.is_some(),
    ) {
        (true, true) => true,
        (true, false) | (false, _) => false,
    }
}
