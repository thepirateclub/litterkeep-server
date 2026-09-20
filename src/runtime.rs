//! Bounded, single-reactor lifecycle primitives.

#[path = "runtime/orchestrator.rs"]
mod orchestrator;

pub use orchestrator::{
    AcquireError, Event, Orchestrator, ResourceId, ResourceLease, ResourceSnapshot, ResourceState,
    TaskId, TaskSnapshot, TaskState, WakeResult,
};
