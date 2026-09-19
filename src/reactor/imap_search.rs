//! Формирование IMAP SEARCH response из согласованного mailbox snapshot.

use super::*;

/// Публикует только числа того пространства адресации, которое выбрал клиент.
pub(super) fn completed(
    session: &mut Session,
    addressing: imap::Addressing,
    key: imap::search::SearchKey,
    tag: String,
    messages: Vec<crate::store::Message>,
) -> io::Result<()> {
    let matches = messages
        .iter()
        .enumerate()
        .filter_map(|(offset, message)| {
            matches_key(message.flags, key).then_some(match addressing {
                imap::Addressing::Sequence => offset.saturating_add(1) as u64,
                imap::Addressing::Uid => message.uid,
            })
        })
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let suffix = matches.join(" ");
    session.push(format!("* SEARCH {suffix}\r\n").as_bytes())?;
    session.push(format!("{tag} OK SEARCH completed\r\n").as_bytes())
}

/// Сопоставляет типизированный key с bitset без строковых protocol-значений.
fn matches_key(flags: crate::mail_flags::MessageFlags, key: imap::search::SearchKey) -> bool {
    use crate::mail_flags::SystemFlag;
    match key {
        imap::search::SearchKey::All => true,
        imap::search::SearchKey::Seen => flags.contains(SystemFlag::Seen),
        imap::search::SearchKey::Unseen => !flags.contains(SystemFlag::Seen),
        imap::search::SearchKey::Deleted => flags.contains(SystemFlag::Deleted),
        imap::search::SearchKey::Undeleted => !flags.contains(SystemFlag::Deleted),
    }
}
