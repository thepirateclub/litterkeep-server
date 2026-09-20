//! Связь фаз исходящего scheduler-а с настоящими ожиданиями оркестратора.

use super::{Orchestrator, Session};
use crate::runtime::{Event, TaskId, TaskState, WakeResult};
use std::io;

/// Атомарно регистрирует ожидание task-а на конкретном completion или timer.
pub(super) fn wait(
    lifecycle: &mut Orchestrator<Session>,
    task: TaskId,
    event: Event,
) -> io::Result<()> {
    let observed = lifecycle.event_version(event);
    match lifecycle.park_on(task, event, observed) {
        WakeResult::Parked => Ok(()),
        WakeResult::AlreadyReady => Err(io::Error::other("scheduler wait raced with signal")),
        WakeResult::MissingTask => Err(io::Error::other("scheduler task is missing")),
    }
}

/// Пробуждает task и проверяет, что реестр действительно перевёл его в Ready.
pub(super) fn wake(
    lifecycle: &mut Orchestrator<Session>,
    task: TaskId,
    event: Event,
) -> io::Result<()> {
    lifecycle.signal(event);
    match lifecycle.task_state(task) {
        Some(TaskState::Ready) => Ok(()),
        Some(_) => Err(io::Error::other(
            "scheduler wake produced invalid task state",
        )),
        None => Err(io::Error::other("scheduler task is missing")),
    }
}
