//! Read-only представления реальных lifecycle records для local control socket.

use super::{ResourceId, ResourceState, TaskId, TaskState};

/// Снимок одной живой задачи без доступа к принадлежащим ей данным.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskSnapshot {
    pub id: TaskId,
    pub state: TaskState,
    pub held_resources: Vec<ResourceId>,
}

/// Снимок одного record ресурса, не раскрывающий его значение или секреты.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceSnapshot {
    pub id: ResourceId,
    pub state: ResourceState,
    pub owner: Option<TaskId>,
}
