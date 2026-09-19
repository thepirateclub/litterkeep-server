use std::collections::{HashMap, VecDeque};
#[path = "lease.rs"]
mod lease;
pub use lease::ResourceLease;
#[path = "model.rs"]
mod model;
pub use model::{AcquireError, Event, ResourceId, ResourceState, TaskId, TaskState, WakeResult};
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

    /// Atomic observe-and-register prevents a lost wake in the check/park gap.
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
        let queue = self.waiters.entry(event).or_default();
        if !queue.contains(&task) {
            queue.push(task);
        }
        WakeResult::Parked
    }

    #[must_use]
    pub fn event_version(&self, event: Event) -> u64 {
        *self.event_versions.get(&event).unwrap_or(&0)
    }

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

    pub fn request_cancel(&mut self, task: TaskId) -> bool {
        let Some(record) = self.tasks.get_mut(&task) else {
            return false;
        };
        if matches!(record.state, TaskState::Completed | TaskState::Failed) {
            return false;
        }
        record.state = TaskState::Cancelling;
        if !self.ready.contains(&task) {
            self.ready.push_back(task);
        }
        true
    }

    #[must_use]
    pub fn task_state(&self, task: TaskId) -> Option<TaskState> {
        self.tasks.get(&task).map(|record| record.state)
    }
    #[must_use]
    pub fn resource_state(&self, resource: ResourceId) -> Option<ResourceState> {
        self.resources.get(&resource).map(|record| record.state)
    }
    pub fn take_ready(&mut self) -> Option<TaskId> {
        self.ready.pop_front()
    }
}
