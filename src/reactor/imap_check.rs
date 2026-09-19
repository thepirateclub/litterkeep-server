//! IMAP CHECK без фиктивной записи в уже fsync-защищённый store.

use super::*;

/// Подтверждает selected mailbox после того, как все mutating операции уже
/// завершили собственный durable commit. Отдельный frame здесь изменил бы
/// состояние без протокольной причины и нарушил бы смысл CHECK.
pub(super) fn execute(session: &mut Session, tag: String) -> io::Result<()> {
    match session.selected_mailbox.is_some() {
        true => session.push(format!("{tag} OK CHECK completed\r\n").as_bytes()),
        false => session.push(format!("{tag} NO select mailbox first\r\n").as_bytes()),
    }
}
