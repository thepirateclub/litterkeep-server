//! Локальный control socket Linux-сервера без доступа к почтовым данным.

#[path = "control/command.rs"]
mod command;
#[path = "control/session.rs"]
mod session;

pub use command::Command;
pub use session::{Connection, Listener, ServiceResult};

use crate::runtime::Orchestrator;

/// Строит безопасный текстовый ответ по реальному lifecycle snapshot.
pub fn response<T>(lifecycle: &mut Orchestrator<T>, request: Command) -> String {
    command::response(lifecycle, request)
}
