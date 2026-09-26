//! Формирование CAPABILITY только из реально исполняемых IMAP возможностей.

use super::*;

/// RFC 5258, RFC 5819 и RFC 7888 extensions объявляются вместе с wire-путями.
const LIST_CAPABILITIES: &str = "LIST-EXTENDED LIST-STATUS LITERAL-";

pub(super) fn write(session: &mut Session, tag: &str) -> io::Result<()> {
    match (session.tls.is_some(), session.can_starttls()) {
        (true, _) => session
            .push(
                format!(
                    "* CAPABILITY IMAP4rev1 IDLE UIDPLUS SPECIAL-USE {LIST_CAPABILITIES} MOVE NAMESPACE UNSELECT ENABLE ESEARCH SEARCHRES APPENDLIMIT={} AUTH=PLAIN SASL-IR\r\n",
                    crate::smtp::MAX_MESSAGE_SIZE
                )
                .as_bytes(),
            )?,
        (false, true) => session.push(
            format!(
                "* CAPABILITY IMAP4rev1 IDLE UIDPLUS SPECIAL-USE {LIST_CAPABILITIES} MOVE NAMESPACE UNSELECT ENABLE ESEARCH SEARCHRES APPENDLIMIT={} STARTTLS LOGINDISABLED\r\n",
                crate::smtp::MAX_MESSAGE_SIZE
            )
            .as_bytes(),
        )?,
        (false, false) => session.push(
            format!(
                "* CAPABILITY IMAP4rev1 IDLE UIDPLUS SPECIAL-USE {LIST_CAPABILITIES} MOVE NAMESPACE UNSELECT ENABLE ESEARCH SEARCHRES APPENDLIMIT={} LOGINDISABLED\r\n",
                crate::smtp::MAX_MESSAGE_SIZE
            )
            .as_bytes(),
        )?,
    }
    session.push(format!("{tag} OK CAPABILITY completed\r\n").as_bytes())
}
