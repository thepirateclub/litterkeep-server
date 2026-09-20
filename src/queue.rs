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
    /// Число реально начатых SMTP попыток, зафиксированное в queue journal.
    pub attempts: u8,
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
            "{} {transaction_id} 0 {sender} {recipient}",
            State::Pending.wire()
        )?;
        file.sync_all()
    }
    /// Фиксирует состояние owned job без изменения immutable envelope данных.
    pub fn transition_owned(&self, delivery: &OwnedDelivery, state: State) -> io::Result<()> {
        validate_token(&delivery.sender)?;
        validate_token(&delivery.recipient)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(
            file,
            "{} {} {} {} {}",
            state.wire(),
            delivery.transaction_id,
            delivery.attempts,
            delivery.sender,
            delivery.recipient
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

    /// Возвращает transaction IDs, для которых журнал содержит owned SMTP job.
    ///
    /// Этот компактный индекс нужен recovery Store: redirect-only transaction не
    /// имеет mailbox link, но остаётся полностью опубликованной транзакцией.
    pub fn owned_transaction_ids(&self) -> io::Result<HashSet<u64>> {
        let source = match fs::read_to_string(&self.path) {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(HashSet::new()),
            Err(error) => return Err(error),
        };
        let mut result = HashSet::new();
        for line in source.lines() {
            let mut fields = line.split_whitespace();
            let state = match fields.next() {
                Some(value) => value,
                None => continue,
            };
            let transaction = match fields.next() {
                Some(value) => value,
                None => continue,
            };
            let _third = match fields.next() {
                Some(value) => value,
                None => continue,
            };
            if fields.next().is_some() {
                State::parse(state)?;
                let transaction_id = transaction
                    .parse()
                    .map_err(|_| invalid("bad owned transaction"))?;
                result.insert(transaction_id);
            }
        }
        Ok(result)
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
            let Some(candidate) = fields.next() else {
                continue;
            };
            let (attempts, sender, recipient) = match candidate.parse::<u8>() {
                Ok(attempts) => match (fields.next(), fields.next(), fields.next()) {
                    (Some(sender), Some(recipient), None) => (attempts, sender, recipient),
                    _ => continue,
                },
                Err(_) => match (fields.next(), fields.next()) {
                    (Some(recipient), None) => (0, candidate, recipient),
                    _ => continue,
                },
            };
            let transaction_id = transaction
                .parse()
                .map_err(|_| invalid("bad owned transaction"))?;
            let state = match State::parse(state)? {
                State::InFlight => State::Deferred,
                state => state,
            };
            let delivery = OwnedDelivery {
                transaction_id,
                attempts,
                sender: sender.into(),
                recipient: recipient.into(),
                state,
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
