//! Ответ приватного RFC 2971 ID extension.

use super::*;

/// Подтверждает ID без раскрытия build, host или системной информации.
pub(super) fn write(session: &mut Session, tag: &str) -> io::Result<()> {
    session.push(b"* ID NIL\r\n")?;
    session.push(format!("{tag} OK ID completed\r\n").as_bytes())
}
