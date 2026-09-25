//! Формирование SEARCH-ошибок без запуска лишнего чтения Store.

use super::*;

/// Отклоняет неизвестную кодировку после проверки selected-state.
pub(super) fn unsupported_charset(
    session: &mut Session,
    tag: &str,
    saves_result: bool,
) -> io::Result<()> {
    if saves_result {
        session.saved_search_uids.clear();
    }
    match session.selected_mailbox.is_some() {
        false => session.push(format!("{tag} NO select mailbox first\r\n").as_bytes()),
        true => session.push(
            format!("{tag} NO [BADCHARSET (US-ASCII UTF-8)] unsupported SEARCH charset\r\n")
                .as_bytes(),
        ),
    }
}
