//! Bounded, single-reactor lifecycle primitives.

#[path = "runtime/orchestrator.rs"]
mod orchestrator;

pub use orchestrator::{
    AcquireError, Event, Orchestrator, ResourceId, ResourceLease, ResourceState, TaskId, TaskState,
    WakeResult,
};
