//! Формирование CAPABILITY только из реально исполняемых IMAP возможностей.

use super::*;

pub(super) fn write(session: &mut Session, tag: &str) -> io::Result<()> {
    match (session.tls.is_some(), session.can_starttls()) {
        (true, _) => session
            .push(b"* CAPABILITY IMAP4rev1 IDLE UIDPLUS SPECIAL-USE MOVE NAMESPACE UNSELECT AUTH=PLAIN SASL-IR\r\n")?,
        (false, true) => session.push(
            b"* CAPABILITY IMAP4rev1 IDLE UIDPLUS SPECIAL-USE MOVE NAMESPACE UNSELECT STARTTLS LOGINDISABLED\r\n",
        )?,
        (false, false) => session
            .push(b"* CAPABILITY IMAP4rev1 IDLE UIDPLUS SPECIAL-USE MOVE NAMESPACE UNSELECT LOGINDISABLED\r\n")?,
    }
    session.push(format!("{tag} OK CAPABILITY completed\r\n").as_bytes())
}
