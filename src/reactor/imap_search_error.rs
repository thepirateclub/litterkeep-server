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

/// RFC 9755 не допускает CHARSET prefix после ENABLE UTF8=ACCEPT.
pub(super) fn charset_after_utf8_accept(
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
            format!("{tag} BAD CHARSET is not allowed after ENABLE UTF8=ACCEPT\r\n").as_bytes(),
        ),
    }
}

/// Обрабатывает extension-specific SEARCH до общей маршрутизации команды.
pub(super) fn reject_utf8_charset(
    session: &mut Session,
    tag: &str,
    command: &imap::Command,
) -> io::Result<bool> {
    match (session.imap_string_mode, command) {
        (
            imap::tokenizer::StringMode::Utf8Accept,
            imap::Command::Search {
                charset_explicit: true,
                result,
                ..
            },
        ) => {
            charset_after_utf8_accept(session, tag, result.saves_result())?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
