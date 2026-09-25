//! Tagged BAD для IMAP-команды с невалидным UTF-8 в wire octets.

use super::Session;
use std::io;

/// Сохраняет безопасный tag запроса и не передаёт повреждённый ввод parser-у.
pub(super) fn respond(session: &mut Session, line: &[u8]) -> io::Result<()> {
    let tag = crate::imap::invalid_utf8_tag::extract(line);
    session.push(format!("{tag} BAD invalid command syntax\r\n").as_bytes())
}
