//! Durable delivery queue journal.
use std::{
    collections::BTreeMap,
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
    /// Добавляет новый факт перехода; старые факты сохраняются для recovery.
    pub fn transition(&self, id: &str, recipient: &str, state: State) -> io::Result<()> {
        self.record(id, recipient, state)
    }
    fn record(&self, id: &str, recipient: &str, state: State) -> io::Result<()> {
        if id.contains(char::is_whitespace) || recipient.contains(char::is_whitespace) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsafe queue token",
            ));
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
                Ok(latest.into_values().collect())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }
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
        let delivery = Queue::open(&d).unwrap().pending().unwrap().pop().unwrap();
        assert_eq!(delivery.state, State::Delivered);
        let _ = fs::remove_dir_all(d);
    }
}
