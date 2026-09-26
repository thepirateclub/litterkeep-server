//! Обработка RFC 5161 ENABLE и возможностей текущего IMAP-соединения.

use super::*;

#[cfg(test)]
#[path = "imap_enable_tests.rs"]
mod tests;

/// Отвечает только в authenticated state; неподдерживаемые возможности игнорируются.
pub(super) fn execute(
    session: &mut Session,
    tag: &str,
    requested: Vec<imap::enable::Capability>,
) -> io::Result<()> {
    match session.authenticated_account.is_some() {
        false => session.push(format!("{tag} NO authenticate first\r\n").as_bytes()),
        true => {
            let utf8_requested = requested
                .iter()
                .any(|capability| matches!(capability, imap::enable::Capability::Utf8Accept));
            if utf8_requested {
                session.imap_string_mode = imap::tokenizer::StringMode::Utf8Accept;
            }
            let enabled = match utf8_requested {
                true => b"* ENABLED UTF8=ACCEPT\r\n".as_slice(),
                false => b"* ENABLED\r\n".as_slice(),
            };
            session.push(enabled)?;
            session.push(format!("{tag} OK ENABLE completed\r\n").as_bytes())
        }
    }
}
