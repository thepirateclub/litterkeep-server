//! Формирование CAPABILITY только из реально исполняемых IMAP возможностей.

use super::*;

pub(super) fn write(session: &mut Session, tag: &str) -> io::Result<()> {
    match (session.tls.is_some(), session.can_starttls()) {
        (true, _) => {
            session.push(b"* CAPABILITY IMAP4rev1 IDLE UIDPLUS SPECIAL-USE AUTH=PLAIN\r\n")?
        }
        (false, true) => session
            .push(b"* CAPABILITY IMAP4rev1 IDLE UIDPLUS SPECIAL-USE STARTTLS LOGINDISABLED\r\n")?,
        (false, false) => {
            session.push(b"* CAPABILITY IMAP4rev1 IDLE UIDPLUS SPECIAL-USE AUTH=PLAIN\r\n")?
        }
    }
    session.push(format!("{tag} OK CAPABILITY completed\r\n").as_bytes())
}
