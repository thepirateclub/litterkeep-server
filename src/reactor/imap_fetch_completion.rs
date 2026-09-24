//! Запись FETCH completion из готового snapshot selected mailbox.

use super::*;
use std::collections::BTreeSet;

pub(super) fn write(
    session: &mut Session,
    addressing: imap::Addressing,
    sequence_set: imap::sequence_set::MessageSet,
    attributes: Vec<imap::FetchAttribute>,
    tag: String,
    saved_uids: &BTreeSet<u64>,
    messages: Vec<crate::store::Message>,
) -> io::Result<()> {
    let maximum = match addressing {
        imap::Addressing::Sequence => messages.len() as u64,
        imap::Addressing::Uid => messages.last().map(|message| message.uid).unwrap_or(0),
    };
    for (offset, message) in messages.iter().enumerate() {
        let number = offset.saturating_add(1);
        let candidate = match addressing {
            imap::Addressing::Sequence => number as u64,
            imap::Addressing::Uid => message.uid,
        };
        if !matches(&sequence_set, candidate, message.uid, maximum, saved_uids) {
            continue;
        }
        if let Err(reason) =
            imap_fetch_response::write(session, number, message, &attributes, addressing)?
        {
            return session.push(format!("{tag} NO {reason}\r\n").as_bytes());
        }
    }
    session.push(format!("{tag} OK FETCH completed\r\n").as_bytes())
}

/// Сопоставляет текущий UID с сохранённым набором либо явным sequence-set.
pub(super) fn matches(
    sequence_set: &imap::sequence_set::MessageSet,
    candidate: u64,
    uid: u64,
    maximum: u64,
    saved_uids: &BTreeSet<u64>,
) -> bool {
    match sequence_set {
        imap::sequence_set::MessageSet::Explicit(explicit) => {
            explicit.contains_bounded(candidate, maximum)
        }
        imap::sequence_set::MessageSet::Saved => saved_uids.contains(&uid),
    }
}
