//! Сбор bounded runtime snapshots без передачи владения registry records.

use super::*;

impl<T> Orchestrator<T> {
    /// Возвращает bounded snapshot реальных lifecycle records без владения ими.
    #[must_use]
    pub fn stats(&self) -> Stats {
        let mut result = Stats {
            ready_queue_depth: self.ready.len(),
            wait_registrations: self.waiters.values().map(Vec::len).sum(),
            ..Stats::default()
        };
        for task in self.tasks.values() {
            match task.state {
                TaskState::Ready => result.ready_tasks += 1,
                TaskState::Waiting { .. } => result.waiting_tasks += 1,
                TaskState::Cancelling => result.cancelling_tasks += 1,
                TaskState::Completed => result.completed_tasks += 1,
                TaskState::Failed => result.failed_tasks += 1,
            }
        }
        for resource in self.resources.values() {
            match resource.state {
                ResourceState::Available => result.available_resources += 1,
                ResourceState::Leased => result.leased_resources += 1,
                ResourceState::Quarantined => result.quarantined_resources += 1,
                ResourceState::Closed => result.closed_resources += 1,
            }
        }
        result
    }

    /// Возвращает только текущие task records, не продлевая их жизнь в registry.
    #[must_use]
    pub fn tasks(&self) -> Vec<TaskSnapshot> {
        self.tasks
            .iter()
            .map(|(id, record)| TaskSnapshot {
                id: *id,
                state: record.state,
                held_resources: record.held_resources.clone(),
            })
            .collect()
    }

    /// Находит одну текущую задачу по generation-protected identifier.
    #[must_use]
    pub fn inspect_task(&self, id: TaskId) -> Option<TaskSnapshot> {
        self.tasks.get(&id).map(|record| TaskSnapshot {
            id,
            state: record.state,
            held_resources: record.held_resources.clone(),
        })
    }

    /// Возвращает состояние resource records, но никогда не их внутреннее value.
    #[must_use]
    pub fn resources(&self) -> Vec<ResourceSnapshot> {
        self.resources
            .iter()
            .map(|(id, record)| ResourceSnapshot {
                id: *id,
                state: record.state,
                owner: record.owner,
            })
            .collect()
    }
}
