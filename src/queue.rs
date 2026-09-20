//! Durable delivery queue journal.
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
#[derive(Debug)]
pub struct Queue {
    path: PathBuf,
}
/// Состояние одного адресата, записываемое в долговечный журнал очереди.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Pending,
    InFlight,
    Deferred,
    Delivered,
    PermanentFailure,
}
impl State {
    fn wire(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InFlight => "in_flight",
            Self::Deferred => "deferred",
            Self::Delivered => "delivered",
            Self::PermanentFailure => "permanent_failure",
        }
    }
    fn parse(value: &str) -> io::Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "in_flight" => Ok(Self::InFlight),
            "deferred" => Ok(Self::Deferred),
            "delivered" => Ok(Self::Delivered),
            "permanent_failure" => Ok(Self::PermanentFailure),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unknown queue state",
            )),
        }
    }
}
/// Одна адресатная запись, а не неразличимая пара строк.
#[derive(Debug, Eq, PartialEq)]
pub struct Delivery {
    pub id: String,
    pub recipient: String,
    pub state: State,
}
/// Очередное задание, принадлежащее immutable blob одной Store transaction.
#[derive(Debug, Eq, PartialEq)]
pub struct OwnedDelivery {
    pub transaction_id: u64,
    pub sender: String,
    pub recipient: String,
    pub state: State,
}
impl Queue {
    pub fn open(root: impl AsRef<Path>) -> io::Result<Self> {
        fs::create_dir_all(root.as_ref())?;
        Ok(Self {
            path: root.as_ref().join("delivery.queue"),
        })
    }
    pub fn enqueue(&self, id: &str, recipient: &str) -> io::Result<()> {
        self.record(id, recipient, State::Pending)
    }
    /// Пишет job до global Store commit. Видимым его делает только committed ID.
    pub fn enqueue_owned(
        &self,
        transaction_id: u64,
        sender: &str,
        recipient: &str,
    ) -> io::Result<()> {
        validate_token(sender)?;
        validate_token(recipient)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(
            file,
            "{} {transaction_id} {sender} {recipient}",
            State::Pending.wire()
        )?;
        file.sync_all()
    }
    /// Добавляет новый факт перехода; старые факты сохраняются для recovery.
    pub fn transition(&self, id: &str, recipient: &str, state: State) -> io::Result<()> {
        self.record(id, recipient, state)
    }
    fn record(&self, id: &str, recipient: &str, state: State) -> io::Result<()> {
        match (
            id.contains(char::is_whitespace),
            recipient.contains(char::is_whitespace),
        ) {
            (true, _) | (false, true) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unsafe queue token",
                ));
            }
            (false, false) => {}
        };
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(f, "{} {id} {recipient}", state.wire())?;
        f.sync_all()
    }
    pub fn pending(&self) -> io::Result<Vec<Delivery>> {
        match fs::read_to_string(&self.path) {
            Ok(s) => {
                let latest: BTreeMap<_, _> = s
                    .lines()
                    .map(|line| {
                        let mut fields = line.split_whitespace();
                        let state = fields.next().ok_or_else(|| {
                            io::Error::new(io::ErrorKind::InvalidData, "bad queue record")
                        })?;
                        let id = fields.next().ok_or_else(|| {
                            io::Error::new(io::ErrorKind::InvalidData, "bad queue record")
                        })?;
                        let recipient = fields.next().ok_or_else(|| {
                            io::Error::new(io::ErrorKind::InvalidData, "bad queue record")
                        })?;
                        Ok(Delivery {
                            id: id.into(),
                            recipient: recipient.into(),
                            state: State::parse(state)?,
                        })
                    })
                    .map(|result| {
                        result.map(|delivery| {
                            ((delivery.id.clone(), delivery.recipient.clone()), delivery)
                        })
                    })
                    .collect::<io::Result<_>>()?;
                Ok(latest
                    .into_values()
                    .filter(|delivery| matches!(delivery.state, State::Pending | State::Deferred))
                    .collect())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    /// Возвращает только jobs, опубликованные общей точкой commit Store.
    pub fn pending_owned(&self, committed: &HashSet<u64>) -> io::Result<Vec<OwnedDelivery>> {
        let source = match fs::read_to_string(&self.path) {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut latest = BTreeMap::new();
        for line in source.lines() {
            let mut fields = line.split_whitespace();
            let state = fields
                .next()
                .ok_or_else(|| invalid("bad owned queue state"))?;
            let transaction = fields.next().ok_or_else(|| invalid("bad owned queue id"))?;
            let (sender, recipient) = match (fields.next(), fields.next(), fields.next()) {
                (Some(sender), Some(recipient), None) => (sender, recipient),
                (Some(_), None, None) | (None, _, None) | (_, _, Some(_)) => continue,
            };
            let transaction_id = transaction
                .parse()
                .map_err(|_| invalid("bad owned transaction"))?;
            let delivery = OwnedDelivery {
                transaction_id,
                sender: sender.into(),
                recipient: recipient.into(),
                state: State::parse(state)?,
            };
            latest.insert((transaction_id, delivery.recipient.clone()), delivery);
        }
        Ok(latest
            .into_values()
            .filter(|delivery| {
                match (
                    committed.contains(&delivery.transaction_id),
                    matches!(delivery.state, State::Pending | State::Deferred),
                ) {
                    (true, true) => true,
                    (true, false) | (false, _) => false,
                }
            })
            .collect())
    }
}

fn validate_token(value: &str) -> io::Result<()> {
    match value.contains(char::is_whitespace) {
        true => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsafe queue token",
        )),
        false => Ok(()),
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_survives_reopen() {
        let d = std::env::temp_dir().join("litterkeep-queue-test");
        let q = Queue::open(&d).unwrap();
        q.enqueue("message-1", "bob@test").unwrap();
        assert_eq!(
            Queue::open(&d).unwrap().pending().unwrap(),
            vec![Delivery {
                id: "message-1".into(),
                recipient: "bob@test".into(),
                state: State::Pending
            }]
        );
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn latest_transition_wins_after_reopen() {
        let d = std::env::temp_dir().join("litterkeep-queue-transition-test");
        let q = Queue::open(&d).unwrap();
        q.enqueue("message-2", "bob@test").unwrap();
        q.transition("message-2", "bob@test", State::Delivered)
            .unwrap();
        assert!(Queue::open(&d).unwrap().pending().unwrap().is_empty());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn owned_delivery_is_invisible_until_store_commit() {
        let d = std::env::temp_dir().join("litterkeep-owned-queue-test");
        let q = Queue::open(&d).expect("queue");
        q.enqueue_owned(7, "alice@test", "remote@test")
            .expect("pending");
        let committed = HashSet::new();
        assert!(q.pending_owned(&committed).expect("uncommitted").is_empty());
        let mut committed = HashSet::new();
        committed.insert(7);
        assert_eq!(
            Queue::open(&d)
                .expect("reopen")
                .pending_owned(&committed)
                .expect("pending"),
            vec![OwnedDelivery {
                transaction_id: 7,
                sender: "alice@test".into(),
                recipient: "remote@test".into(),
                state: State::Pending,
            }]
        );
        let _ = fs::remove_dir_all(d);
    }
}
