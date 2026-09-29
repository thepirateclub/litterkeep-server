//! Bounded worker исходящего SMTP, владеющий pool и не имеющий доступа к Store.

use crate::{outbound::DeliveryOutcome, outbound_route};
use std::io;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::{self, JoinHandle};

/// Immutable job, выданный transport только после durable InFlight transition.
#[derive(Debug)]
pub struct Job {
    pub id: u64,
    pub sender: String,
    pub recipient: String,
    pub payload: Vec<u8>,
    /// Сохранённые исходные MAIL/RCPT DSN параметры forwarding job-а.
    pub dsn_metadata: Option<crate::queue::DsnDeliveryMetadata>,
}

/// Backpressure outcome не возвращает крупный SMTP payload обратно caller-у.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitError {
    Full,
    Disconnected,
}

/// Completion сохраняет outcome отдельно от владения connection pool.
#[derive(Debug)]
pub struct Completion {
    pub id: u64,
    pub result: io::Result<DeliveryOutcome>,
    /// Исходный SMTP reply, который можно безопасно включить в failure DSN.
    pub diagnostic: Option<crate::smtp_reply_diagnostic::SmtpReplyDiagnostic>,
}

/// Один bounded transport owner. Он не создаёт thread для каждого job.
pub struct OutboundWorker {
    jobs: SyncSender<Job>,
    completions: Receiver<Completion>,
    thread: JoinHandle<io::Result<()>>,
}

impl OutboundWorker {
    /// Создаёт transport owner с ограниченными job queue и pool capacity.
    pub fn start(
        address: impl Into<String>,
        queue_capacity: usize,
        pool_capacity: usize,
    ) -> io::Result<Self> {
        Self::start_route(
            outbound_route::Route::plain(&address.into())?,
            queue_capacity,
            pool_capacity,
        )
    }

    pub fn start_route(
        route: outbound_route::Route,
        queue_capacity: usize,
        pool_capacity: usize,
    ) -> io::Result<Self> {
        Self::start_route_with_notifier(route, queue_capacity, pool_capacity, || {})
    }

    /// Создаёт worker с коротким notifier-ом для внешнего event loop.
    ///
    /// Notifier не передаёт completion и не является его источником истины:
    /// после spurious либо coalesced wake reactor читает bounded channel.
    pub fn start_with_notifier<F>(
        address: impl Into<String>,
        queue_capacity: usize,
        pool_capacity: usize,
        notifier: F,
    ) -> io::Result<Self>
    where
        F: FnMut() + Send + 'static,
    {
        let route = outbound_route::Route::plain(&address.into())?;
        Self::start_route_with_notifier(route, queue_capacity, pool_capacity, notifier)
    }

    pub fn start_route_with_notifier<F>(
        route: outbound_route::Route,
        queue_capacity: usize,
        pool_capacity: usize,
        notifier: F,
    ) -> io::Result<Self>
    where
        F: FnMut() + Send + 'static,
    {
        let _timeouts = crate::outbound::SmtpTimeouts::from_environment()?;
        let pool = outbound_route::Pool::new(route, pool_capacity)?;
        let (jobs, incoming) = mpsc::sync_channel(queue_capacity);
        let (outgoing, completions) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("litterkeep-outbound-smtp".into())
            .spawn(move || run(pool, incoming, outgoing, notifier))?;
        Ok(Self {
            jobs,
            completions,
            thread,
        })
    }

    /// Не ждёт свободного slot: reactor сохраняет backpressure в своём state.
    pub fn submit(&self, job: Job) -> Result<(), SubmitError> {
        self.jobs.try_send(job).map_err(|error| match error {
            TrySendError::Full(_) => SubmitError::Full,
            TrySendError::Disconnected(_) => SubmitError::Disconnected,
        })
    }

    /// Возвращает готовый completion без polling loop в ожидании transport-а.
    pub fn try_completion(&self) -> Result<Completion, TryRecvError> {
        self.completions.try_recv()
    }

    /// Закрывает вход jobs и присоединяет единственный transport thread.
    pub fn shutdown(self) -> io::Result<()> {
        drop(self.jobs);
        self.thread
            .join()
            .map_err(|_| io::Error::other("outbound worker thread panicked"))?
    }
}

/// Последовательно исполняет SMTP jobs, сохраняя pool owner в одном thread.
fn run<F>(
    mut pool: outbound_route::Pool,
    incoming: Receiver<Job>,
    outgoing: mpsc::Sender<Completion>,
    mut notifier: F,
) -> io::Result<()>
where
    F: FnMut(),
{
    for job in incoming {
        let attempt = pool.deliver_detailed_with_dsn(
            &job.sender,
            &job.recipient,
            &job.payload,
            job.dsn_metadata.as_ref(),
        );
        let (result, diagnostic) = match attempt {
            Ok(attempt) => (Ok(attempt.outcome), attempt.diagnostic),
            Err(error) => (Err(error), None),
        };
        if outgoing
            .send(Completion {
                id: job.id,
                result,
                diagnostic,
            })
            .is_err()
        {
            break;
        }
        notifier();
    }
    pool.shutdown()
}
