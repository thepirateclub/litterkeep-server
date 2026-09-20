//! Bounded worker исходящего SMTP, владеющий pool и не имеющий доступа к Store.

use crate::{outbound::DeliveryOutcome, outbound_pool::ConnectionPool};
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
}

/// Completion сохраняет outcome отдельно от владения connection pool.
#[derive(Debug)]
pub struct Completion {
    pub id: u64,
    pub result: io::Result<DeliveryOutcome>,
}

/// Один bounded transport owner. Он не создаёт thread для каждого job.
pub struct OutboundWorker {
    jobs: SyncSender<Job>,
    completions: Receiver<Completion>,
    thread: JoinHandle<()>,
}

impl OutboundWorker {
    /// Создаёт transport owner с ограниченными job queue и pool capacity.
    pub fn start(
        address: impl Into<String>,
        queue_capacity: usize,
        pool_capacity: usize,
    ) -> io::Result<Self> {
        Self::start_with_notifier(address, queue_capacity, pool_capacity, || {})
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
        let pool = ConnectionPool::new(address, pool_capacity)?;
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
    pub fn submit(&self, job: Job) -> Result<(), TrySendError<Job>> {
        self.jobs.try_send(job)
    }

    /// Возвращает готовый completion без polling loop в ожидании transport-а.
    pub fn try_completion(&self) -> Result<Completion, TryRecvError> {
        self.completions.try_recv()
    }

    /// Закрывает вход jobs и присоединяет единственный transport thread.
    pub fn shutdown(self) {
        drop(self.jobs);
        let _ = self.thread.join();
    }
}

/// Последовательно исполняет SMTP jobs, сохраняя pool owner в одном thread.
fn run<F>(
    mut pool: ConnectionPool,
    incoming: Receiver<Job>,
    outgoing: mpsc::Sender<Completion>,
    mut notifier: F,
) where
    F: FnMut(),
{
    for job in incoming {
        let result = pool.deliver(&job.sender, &job.recipient, &job.payload);
        if outgoing.send(Completion { id: job.id, result }).is_err() {
            return;
        }
        notifier();
    }
}
