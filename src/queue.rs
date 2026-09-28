//! Durable delivery queue journal.
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[path = "queue_enqueue.rs"]
mod queue_enqueue;
#[cfg(test)]
#[path = "queue_origin_tests.rs"]
mod queue_origin_tests;
#[path = "queue_owned.rs"]
mod queue_owned;
#[derive(Debug)]
pub struct Queue {
    path: PathBuf,
}
pub(super) const MAX_ORIGIN_RECIPIENTS: usize = 100;

/// Кодирует origin ordinal-ы для shared durable journal формата.
pub(crate) fn encode_recipient_indices(indices: &[usize]) -> String {
    queue_owned::encode_recipient_indices(indices)
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

/// DSN policy одного исходного получателя, восстановленная Store по ordinal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DsnDeliveryMetadata {
    /// Параметры MAIL, общие для исходной SMTP transaction.
    pub mail: crate::smtp::MailDsn,
    /// Параметры RCPT, выбранные для передачи одному outbound route.
    pub recipient: Option<crate::smtp::RecipientDsn>,
    /// Все исходные RCPT policy, сведённые в один локальный Sieve redirect.
    pub origin_recipients: Vec<Option<crate::smtp::RecipientDsn>>,
}

/// Идемпотентная идентичность failure DSN для одной durable queue job.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DsnReportKey {
    /// Глобальный ID исходной Store transaction.
    pub transaction_id: u64,
    /// Конечный адрес queue route, чтобы различать несколько redirect jobs.
    pub recipient: String,
    /// Все исходные SMTP RCPT, сведённые в этот durable маршрут.
    pub original_recipient_indices: Vec<usize>,
    /// Терминальная причина, включённая в сохраняемый report.
    pub failure: crate::dsn::Failure,
}

/// Идемпотентная идентичность committed SMTP local-success уведомления.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DsnSuccessKey {
    /// Глобальная transaction ID исходной local delivery.
    pub transaction_id: u64,
    /// Нулевой ordinal RCPT из исходного SMTP envelope.
    pub recipient_index: usize,
}

/// Идемпотентная запись delivered report для committed local-success intent.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DsnSuccessReportKey {
    /// Глобальный ID исходного письма, ставшего доступным получателю.
    pub source_transaction_id: u64,
    /// Нулевой ordinal получателя в исходном SMTP envelope.
    pub recipient_index: usize,
}

/// Восстановленный исходный контекст, необходимый consumer-у SUCCESS DSN.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DsnSuccessContext {
    /// Envelope reverse-path исходной SMTP transaction.
    pub reverse_path: String,
    /// Исходный адрес RCPT соответствующего intent ordinal.
    pub recipient: String,
    /// Побайтное исходное сообщение из immutable blob.
    pub original: Vec<u8>,
    /// MAIL/RCPT DSN policy, восстановленная из commit-linked sidecar.
    pub metadata: DsnDeliveryMetadata,
}

/// Очередное задание, принадлежащее immutable blob одной Store transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedDelivery {
    pub transaction_id: u64,
    /// Число реально начатых SMTP попыток, зафиксированное в queue journal.
    pub attempts: u8,
    /// Unix epoch seconds, раньше которых deferred job не выдаётся worker-у.
    pub next_attempt_epoch: u64,
    pub sender: String,
    pub recipient: String,
    /// Индексы SMTP RCPT, чьи Sieve redirects сведены в одну queue job.
    pub original_recipient_indices: Vec<usize>,
    /// Typed DSN данные исходной transaction, подставляемые Store при чтении.
    pub dsn_metadata: Option<Box<DsnDeliveryMetadata>>,
    /// Failure DSN, который ещё должен быть опубликован после recovery.
    pub dsn_failure: Option<crate::dsn::Failure>,
    /// Ответ удалённого SMTP peer, зафиксированный вместе с failure notice.
    pub(crate) dsn_diagnostic: Option<crate::smtp_reply_diagnostic::SmtpReplyDiagnostic>,
    pub state: State,
}

impl OwnedDelivery {
    /// Формирует стабильный ключ отчёта из durable identity queue job.
    pub fn dsn_report_key(&self) -> Option<DsnReportKey> {
        self.dsn_failure.map(|failure| DsnReportKey {
            transaction_id: self.transaction_id,
            recipient: self.recipient.clone(),
            original_recipient_indices: self.original_recipient_indices.clone(),
            failure,
        })
    }
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
}

/// Кодирует durable failure notice без свободного текстового значения.
fn encode_dsn_failure(failure: Option<crate::dsn::Failure>) -> &'static str {
    match failure {
        Some(value) => value.journal_tag(),
        None => "-",
    }
}

/// Представляет null reverse-path отдельным токеном whitespace-журнала.
fn encode_sender(value: &str) -> &str {
    match value.is_empty() {
        true => "<>",
        false => value,
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
