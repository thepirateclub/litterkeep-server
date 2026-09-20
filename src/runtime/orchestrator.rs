use std::collections::{HashMap, VecDeque};
#[path = "lease.rs"]
mod lease;
pub use lease::ResourceLease;
#[path = "model.rs"]
mod model;
pub use model::{AcquireError, Event, ResourceId, ResourceState, TaskId, TaskState, WakeResult};
#[path = "event_control.rs"]
mod event_control;
#[path = "stats.rs"]
mod stats;
#[path = "task_control.rs"]
mod task_control;
pub use stats::Stats;
#[path = "introspection.rs"]
mod introspection;
pub use introspection::{ResourceSnapshot, TaskSnapshot};
#[path = "introspection_api.rs"]
mod introspection_api;
#[cfg(test)]
#[path = "orchestrator_tests.rs"]
mod tests;

#[derive(Debug)]
struct TaskRecord {
    state: TaskState,
    held_resources: Vec<ResourceId>,
}

#[derive(Debug)]
struct ResourceRecord<T> {
    state: ResourceState,
    owner: Option<TaskId>,
    value: Option<T>,
}

/// Owns registrations and scheduler metadata, while a lease owns the value.
#[derive(Debug)]
pub struct Orchestrator<T> {
    next_task_slot: u64,
    next_resource_slot: u64,
    tasks: HashMap<TaskId, TaskRecord>,
    resources: HashMap<ResourceId, ResourceRecord<T>>,
    event_versions: HashMap<Event, u64>,
    waiters: HashMap<Event, Vec<TaskId>>,
    ready: VecDeque<TaskId>,
}

impl<T> Default for Orchestrator<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Orchestrator<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_task_slot: 0,
            next_resource_slot: 0,
            tasks: HashMap::new(),
            resources: HashMap::new(),
            event_versions: HashMap::new(),
            waiters: HashMap::new(),
            ready: VecDeque::new(),
        }
    }

    pub fn spawn_task(&mut self) -> TaskId {
        let id = TaskId {
            slot: self.next_task_slot,
            generation: 1,
        };
        self.next_task_slot = self
            .next_task_slot
            .checked_add(1)
            .expect("task id exhausted");
        self.tasks.insert(
            id,
            TaskRecord {
                state: TaskState::Ready,
                held_resources: Vec::new(),
            },
        );
        self.ready.push_back(id);
        id
    }

    pub fn register_resource(&mut self, value: T) -> ResourceId {
        let id = ResourceId {
            slot: self.next_resource_slot,
            generation: 1,
        };
        self.next_resource_slot = self
            .next_resource_slot
            .checked_add(1)
            .expect("resource id exhausted");
        self.resources.insert(
            id,
            ResourceRecord {
                state: ResourceState::Available,
                owner: None,
                value: Some(value),
            },
        );
        id
    }

    pub fn acquire(
        &mut self,
        task: TaskId,
        resource: ResourceId,
    ) -> Result<ResourceLease<T>, AcquireError> {
        if !self.tasks.contains_key(&task) {
            return Err(AcquireError::MissingTask);
        }
        let record = self
            .resources
            .get_mut(&resource)
            .ok_or(AcquireError::MissingResource)?;
        if record.state != ResourceState::Available {
            return Err(AcquireError::NotAvailable(record.state));
        }
        let value = record
            .value
            .take()
            .expect("available resource must have a value");
        record.state = ResourceState::Leased;
        record.owner = Some(task);
        self.tasks
            .get_mut(&task)
            .expect("checked above")
            .held_resources
            .push(resource);
        Ok(ResourceLease {
            resource,
            owner: task,
            value: Some(value),
        })
    }

    /// Returns a resource only after identity and actual owner both match.
    pub fn release(&mut self, mut lease: ResourceLease<T>) -> Result<(), AcquireError> {
        let record = self
            .resources
            .get_mut(&lease.resource)
            .ok_or(AcquireError::MissingResource)?;
        match (
            record.state == ResourceState::Leased,
            record.owner == Some(lease.owner),
        ) {
            (true, true) => {}
            (false, _) | (true, false) => {
                return Err(AcquireError::NotAvailable(record.state));
            }
        }
        record.value = lease.value.take();
        record.owner = None;
        record.state = ResourceState::Available;
        if let Some(task) = self.tasks.get_mut(&lease.owner) {
            task.held_resources.retain(|id| *id != lease.resource);
        }
        Ok(())
    }

    /// An uncertain connection is dropped and never returned to a pool.
    pub fn quarantine(&mut self, mut lease: ResourceLease<T>) -> Result<(), AcquireError> {
        let record = self
            .resources
            .get_mut(&lease.resource)
            .ok_or(AcquireError::MissingResource)?;
        match (
            record.state == ResourceState::Leased,
            record.owner == Some(lease.owner),
        ) {
            (true, true) => {}
            (false, _) | (true, false) => {
                return Err(AcquireError::NotAvailable(record.state));
            }
        }
        drop(lease.value.take());
        record.owner = None;
        record.state = ResourceState::Quarantined;
        if let Some(task) = self.tasks.get_mut(&lease.owner) {
            task.held_resources.retain(|id| *id != lease.resource);
        }
        Ok(())
    }

    /// Закрывает реальный ресурс и удаляет terminal metadata без утечки registry.
    pub fn close(&mut self, mut lease: ResourceLease<T>) -> Result<(), AcquireError> {
        let state = self
            .resources
            .get(&lease.resource)
            .ok_or(AcquireError::MissingResource)?;
        match (
            state.state == ResourceState::Leased,
            state.owner == Some(lease.owner),
        ) {
            (true, true) => {}
            (false, _) | (true, false) => {
                return Err(AcquireError::NotAvailable(state.state));
            }
        }
        drop(lease.value.take());
        self.resources.remove(&lease.resource);
        if let Some(task) = self.tasks.get_mut(&lease.owner) {
            task.held_resources.retain(|id| *id != lease.resource);
        }
        self.ready.retain(|task| *task != lease.owner);
        self.tasks.remove(&lease.owner);
        for waiters in self.waiters.values_mut() {
            waiters.retain(|task| *task != lease.owner);
        }
        self.retire_task_events(lease.owner);
        Ok(())
    }

    /// Число реально зарегистрированных ресурсов, используемое admission policy.
    #[must_use]
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    /// Число живых задач для проверки отсутствия terminal metadata leak.
    #[must_use]
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    #[must_use]
    pub fn task_state(&self, task: TaskId) -> Option<TaskState> {
        self.tasks.get(&task).map(|record| record.state)
    }
    #[must_use]
    pub fn resource_state(&self, resource: ResourceId) -> Option<ResourceState> {
        self.resources.get(&resource).map(|record| record.state)
    }
}
