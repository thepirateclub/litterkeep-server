//! Регистрация, пробуждение и bounded retirement lifecycle-событий.

use super::{Event, Orchestrator, TaskId, TaskState, WakeResult};

impl<T> Orchestrator<T> {
    /// Atomic observe-and-register исключает lost wake в промежутке check/park.
    pub fn park_on(&mut self, task: TaskId, event: Event, observed_version: u64) -> WakeResult {
        let Some(record) = self.tasks.get_mut(&task) else {
            return WakeResult::MissingTask;
        };
        let current = *self.event_versions.get(&event).unwrap_or(&0);
        if current != observed_version {
            record.state = TaskState::Ready;
            return WakeResult::AlreadyReady;
        }
        record.state = TaskState::Waiting {
            observed_event_version: current,
        };
        self.ready.retain(|ready| *ready != task);
        let queue = self.waiters.entry(event).or_default();
        if !queue.contains(&task) {
            queue.push(task);
        }
        WakeResult::Parked
    }

    /// Возвращает version наблюдаемого события либо начальную нулевую версию.
    #[must_use]
    pub fn event_version(&self, event: Event) -> u64 {
        *self.event_versions.get(&event).unwrap_or(&0)
    }

    /// Пробуждает всех зарегистрированных владельцев ровно один раз.
    pub fn signal(&mut self, event: Event) {
        let version = self.event_versions.entry(event).or_insert(0);
        *version = version.checked_add(1).expect("event version exhausted");
        for task in self.waiters.remove(&event).unwrap_or_default() {
            match self.tasks.get_mut(&task) {
                Some(record) if matches!(record.state, TaskState::Waiting { .. }) => {
                    record.state = TaskState::Ready;
                    if !self.ready.contains(&task) {
                        self.ready.push_back(task);
                    }
                }
                Some(_) | None => {}
            }
        }
    }

    /// Удаляет завершённое событие только после снятия всех registrations.
    ///
    /// Одноразовые completion ID иначе образовали бы неограниченную историю в
    /// `event_versions`, хотя соответствующая операция и владелец уже живы в
    /// следующей фазе. Событие без потребителя остаётся для защиты lost wake.
    pub fn retire_event(&mut self, event: Event) -> bool {
        if self.waiters.contains_key(&event) {
            return false;
        }
        self.event_versions.remove(&event).is_some()
    }

    /// Очищает task-scoped timer/cancel metadata при окончательном retirement.
    pub(super) fn retire_task_events(&mut self, task: TaskId) {
        self.event_versions.retain(|event, _| match event {
            Event::Timer(owner) | Event::Cancel(owner) => *owner != task,
            _ => true,
        });
        self.waiters.retain(|event, waiters| match event {
            Event::Timer(owner) | Event::Cancel(owner) => *owner != task,
            _ => !waiters.is_empty(),
        });
    }
}
