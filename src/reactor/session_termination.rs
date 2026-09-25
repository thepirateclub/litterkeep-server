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
        let terminated = match entry.as_mut() {
            Some(lease) => match lease.get_mut().termination {
                Some(super::SessionTermination::Pop3Quit) => finish_pop3_quit(lease),
                Some(_) => true,
                None => false,
            },
            None => false,
        };
        if terminated {
            session_close::close(entry, lifecycle, maildrops)?;
        }
    }
    Ok(())
}

/// Дренирует POP3 +OK перед TLS alert и закрывает только после его flush.
fn finish_pop3_quit(lease: &mut SessionLease) -> bool {
    let session = lease.get_mut();
    match (session.output.is_empty(), session.wire_output.is_empty()) {
        (false, _) | (_, false) => false,
        (true, true) => match session.tls.is_some() {
            true => {
                session.begin_close_notify();
                match session.tls.as_ref() {
                    Some(tls) => !tls.wants_write(),
                    None => true,
                }
            }
            false => true,
        },
    }
}
