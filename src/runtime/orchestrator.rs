use std::collections::{HashMap, VecDeque};
#[path = "lease.rs"]
mod lease;
pub use lease::ResourceLease;
#[cfg(test)]
#[path = "orchestrator_tests.rs"]
mod tests;

/// An opaque task identity, deliberately unrelated to a descriptor number.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TaskId {
    slot: u64,
    generation: u64,
}

/// An opaque resource identity. Generations make stale completions harmless.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResourceId {
    slot: u64,
    generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskState {
    Ready,
    Waiting { observed_event_version: u64 },
    Cancelling,
    Completed,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceState {
    Available,
    Leased,
    Quarantined,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Event {
    SocketReadable(u64),
    SocketWritable(u64),
    MailboxRevision(u64),
    Completion(u64),
    Cancel(TaskId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WakeResult {
    Parked,
    AlreadyReady,
    MissingTask,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcquireError {
    MissingTask,
    MissingResource,
    NotAvailable(ResourceState),
}

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
        if record.state != ResourceState::Leased || record.owner != Some(lease.owner) {
            return Err(AcquireError::NotAvailable(record.state));
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
        if record.state != ResourceState::Leased || record.owner != Some(lease.owner) {
            return Err(AcquireError::NotAvailable(record.state));
        }
        drop(lease.value.take());
        record.owner = None;
        record.state = ResourceState::Quarantined;
        if let Some(task) = self.tasks.get_mut(&lease.owner) {
            task.held_resources.retain(|id| *id != lease.resource);
        }
        Ok(())
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
            if let Some(record) = self.tasks.get_mut(&task)
                && matches!(record.state, TaskState::Waiting { .. })
            {
                record.state = TaskState::Ready;
                if !self.ready.contains(&task) {
                    self.ready.push_back(task);
                }
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
