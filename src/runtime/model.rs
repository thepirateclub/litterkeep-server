//! Типизированная модель состояний оркестратора жизненного цикла.

/// Непрозрачный идентификатор задачи, не связанный с номером дескриптора.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TaskId {
    pub(super) slot: u64,
    pub(super) generation: u64,
}

impl TaskId {
    /// Разбирает внешний стабильный идентификатор `slot:generation`.
    pub fn parse(value: &str) -> Option<Self> {
        let (slot, generation) = value.split_once(':')?;
        Some(Self {
            slot: slot.parse().ok()?,
            generation: generation.parse().ok()?,
        })
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}:{}", self.slot, self.generation)
    }
}

/// Непрозрачный идентификатор ресурса с защитой от устаревших завершений.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResourceId {
    pub(super) slot: u64,
    pub(super) generation: u64,
}

impl std::fmt::Display for ResourceId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}:{}", self.slot, self.generation)
    }
}

/// Состояние задачи, которым управляет оркестратор.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskState {
    Ready,
    Waiting { observed_event_version: u64 },
    Cancelling,
    Completed,
    Failed,
}

/// Состояние зарегистрированного ресурса.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceState {
    Available,
    Leased,
    Quarantined,
    Closed,
}

/// Событие, на котором задача может ожидать пробуждения.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Event {
    SocketReadable(u64),
    SocketWritable(u64),
    MailboxRevision(u64),
    Completion(u64),
    Cancel(TaskId),
}

/// Результат атомарной регистрации ожидания.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WakeResult {
    Parked,
    AlreadyReady,
    MissingTask,
}

/// Ошибка выдачи, возврата или закрытия ресурса.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcquireError {
    MissingTask,
    MissingResource,
    NotAvailable(ResourceState),
}
