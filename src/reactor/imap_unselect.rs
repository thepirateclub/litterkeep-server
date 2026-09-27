//! Сброс выбранной IMAP mailbox без физического удаления \Deleted сообщений.

use super::*;

/// Освобождает session projection и возвращает соединение в authenticated state.
pub(super) fn execute(session: &mut Session, tag: String) -> io::Result<()> {
    match session.selected_mailbox.take() {
        Some(_) => {
            clear_selected_projection(session);
            session.push(format!("{tag} OK UNSELECT completed\r\n").as_bytes())
        }
        None => {
            session.push(format!("{tag} BAD UNSELECT requires selected mailbox\r\n").as_bytes())
        }
    }
}

/// Закрывает прежний выбор до попытки SELECT/EXAMINE новой mailbox.
pub(super) fn close_for_select(session: &mut Session) -> io::Result<()> {
    match session.selected_mailbox.take() {
        Some(_) => {
            clear_selected_projection(session);
            session.push(b"* OK [CLOSED] Previous mailbox is now closed\r\n")
        }
        None => Ok(()),
    }
}

/// Очищает только проекцию выбранной папки, не удаляя \\Deleted сообщения.
fn clear_selected_projection(session: &mut Session) {
    session.selected_exists = 0;
    session.selected_recent.clear();
    session.saved_search_uids.clear();
    session.selected_read_only = false;
    session.imap_idle = None;
}
