//! Обработка RFC 5161 ENABLE и возможностей текущего IMAP-соединения.

use super::*;

/// Отвечает только в authenticated state; неподдерживаемые возможности игнорируются.
pub(super) fn execute(
    session: &mut Session,
    tag: &str,
    _requested: Vec<imap::enable::Capability>,
) -> io::Result<()> {
    match session.authenticated_account.is_some() {
        false => session.push(format!("{tag} NO authenticate first\r\n").as_bytes()),
        true => {
            session.push(b"* ENABLED\r\n")?;
            session.push(format!("{tag} OK ENABLE completed\r\n").as_bytes())
        }
    }
}
