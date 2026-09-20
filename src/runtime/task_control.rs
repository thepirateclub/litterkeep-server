//! Отмена и удаление задач без нарушения владения ресурсами.

use super::{Orchestrator, TaskId, TaskState};

impl<T> Orchestrator<T> {
    /// Переводит живую задачу в cooperative cancellation и снимает ожидания.
    pub fn request_cancel(&mut self, task: TaskId) -> bool {
        let Some(record) = self.tasks.get_mut(&task) else {
            return false;
        };
        if matches!(record.state, TaskState::Completed | TaskState::Failed) {
            return false;
        }
        record.state = TaskState::Cancelling;
        for waiters in self.waiters.values_mut() {
            waiters.retain(|waiting| *waiting != task);
        }
        if !self.ready.contains(&task) {
            self.ready.push_back(task);
        }
        true
    }

    /// Удаляет terminal task только когда за ней не осталось реальных ресурсов.
    pub fn retire_task(&mut self, task: TaskId) -> bool {
        let Some(record) = self.tasks.get(&task) else {
            return false;
        };
        if !record.held_resources.is_empty() {
            return false;
        }
        self.tasks.remove(&task);
        self.ready.retain(|ready| *ready != task);
        for waiters in self.waiters.values_mut() {
            waiters.retain(|waiting| *waiting != task);
        }
        self.retire_task_events(task);
        true
    }

    pub fn take_ready(&mut self) -> Option<TaskId> {
        self.ready.pop_front()
    }
}
