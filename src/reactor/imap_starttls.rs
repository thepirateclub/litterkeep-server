//! Переход IMAP-сессии из plaintext в TLS до аутентификации.

use super::*;

/// Начинает TLS upgrade только на разрешённой границе протокольного состояния.
pub(super) fn begin(session: &mut Session, tag: &str) -> io::Result<()> {
    match (
        session.can_starttls(),
        session.authenticated_account.is_none(),
    ) {
        (true, true) => {
            session.push(format!("{tag} OK Begin TLS negotiation now\r\n").as_bytes())?;
            session.request_starttls();
            Ok(())
        }
        (true, false) => session.push(format!("{tag} BAD invalid state\r\n").as_bytes()),
        (false, _) => session.push(format!("{tag} BAD STARTTLS unavailable\r\n").as_bytes()),
    }
}
