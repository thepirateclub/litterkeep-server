//! Обслуживание local control connections в контексте mail reactor-а.

use super::*;

/// Обслуживает control запрос тем же readiness-driven путём без отдельного thread.
pub(super) fn service_control(
    ready: crate::platform::linux_poll::Ready,
    slot: usize,
    sessions: &mut [Option<crate::control::Connection>],
    lifecycle: &Orchestrator<Session>,
) -> io::Result<()> {
    let Some(entry) = sessions.get_mut(slot) else {
        return Ok(());
    };
    if ready.closed_or_error {
        *entry = None;
        return Ok(());
    }
    let Some(connection) = entry.as_mut() else {
        return Ok(());
    };
    match connection.service(ready.readable, ready.writable, lifecycle)? {
        crate::control::ServiceResult::Keep => Ok(()),
        crate::control::ServiceResult::Close => {
            *entry = None;
            Ok(())
        }
    }
}
