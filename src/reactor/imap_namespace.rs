//! Описание персонального пространства имён выбранной учётной записи.

use super::*;

/// Возвращает пустой personal prefix с используемым сервером разделителем.
pub(super) fn write(session: &mut Session, tag: &str) -> io::Result<()> {
    match session.authenticated_account.is_some() {
        true => {
            session.push(b"* NAMESPACE ((\"\" \"/\")) NIL NIL\r\n")?;
            session.push(format!("{tag} OK NAMESPACE completed\r\n").as_bytes())
        }
        false => session.push(format!("{tag} NO authenticate first\r\n").as_bytes()),
    }
}
