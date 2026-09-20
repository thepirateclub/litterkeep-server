//! Изоляция протокольной ошибки конкретного владельца TCP-сессии.

use super::super::{MaildropLeases, Orchestrator, Session, SessionLease, session_close};
use std::io;

/// Закрывает только сессии, локально помеченные протокольной защитой.
///
/// Проход выполняется после обработки всей готовой пачки событий. Поэтому
/// рассылка одному медленному IDLE-подписчику не прерывает доставку события
/// другим подписчикам, а ресурс проблемного клиента освобождается тем же
/// lifecycle-путём, что EOF и cooperative cancellation.
pub(in crate::reactor) fn close_terminated(
    sessions: &mut [Option<SessionLease>],
    lifecycle: &mut Orchestrator<Session>,
    maildrops: &mut MaildropLeases,
) -> io::Result<()> {
    for entry in sessions {
        let terminated = entry
            .as_ref()
            .is_some_and(|lease| lease.get().termination.is_some());
        if terminated {
            session_close::close(entry, lifecycle, maildrops)?;
        }
    }
    Ok(())
}
