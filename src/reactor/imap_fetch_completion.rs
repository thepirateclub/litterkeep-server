//! Запись FETCH completion из готового snapshot selected mailbox.

use super::*;
use std::collections::BTreeSet;

pub(super) struct Completion<'a> {
    pub(super) addressing: imap::Addressing,
    pub(super) sequence_set: imap::sequence_set::MessageSet,
    pub(super) attributes: Vec<imap::FetchAttribute>,
    pub(super) partial: Option<imap::partial_range::PartialRange>,
    pub(super) tag: String,
    pub(super) saved_uids: &'a BTreeSet<u64>,
    pub(super) messages: Vec<crate::store::Message>,
    pub(super) changed_uids: Option<BTreeSet<u64>>,
}

pub(super) fn write(session: &mut Session, completion: Completion<'_>) -> io::Result<()> {
    write_internal(session, completion)
}

/// Завершает BODY FETCH с FLAGS из успешного durable Seen update.
pub(super) fn write_seen_update(
    session: &mut Session,
    completion: Completion<'_>,
) -> io::Result<()> {
    write_internal(session, completion)
}

fn write_internal(session: &mut Session, completion: Completion<'_>) -> io::Result<()> {
    let Completion {
        addressing,
        sequence_set,
        attributes,
        partial,
        tag,
        saved_uids,
        messages,
        changed_uids,
    } = completion;
    let maximum = match addressing {
        imap::Addressing::Sequence => messages.len() as u64,
        imap::Addressing::Uid => messages.last().map(|message| message.uid).unwrap_or(0),
    };
    let selected_positions = super::imap_fetch_selection::positions(
        addressing,
        &sequence_set,
        partial.as_ref(),
        &messages,
        saved_uids,
    );
    let mut rank = 0_usize;
    for (offset, message) in messages.iter().enumerate() {
        let number = offset.saturating_add(1);
        let candidate = match addressing {
            imap::Addressing::Sequence => number as u64,
            imap::Addressing::Uid => message.uid,
        };
        if !matches(&sequence_set, candidate, message.uid, maximum, saved_uids) {
            continue;
        }
        let selected = selected_positions.contains(&rank);
        rank = rank.saturating_add(1);
        if !selected {
            continue;
        }
        let response = match &changed_uids {
            Some(uids) if uids.contains(&message.uid) => {
                imap_fetch_seen_response::write(session, number, message, &attributes, addressing)?
            }
            _ => imap_fetch_response::write(session, number, message, &attributes, addressing)?,
        };
        if let Err(reason) = response {
            let response = match reason {
                crate::imap_fetch::FetchError::UnknownContentTransferEncoding => {
                    format!("{tag} NO [UNKNOWN-CTE] unsupported MIME encoding\r\n")
                }
                crate::imap_fetch::FetchError::InvalidSection => {
                    format!("{tag} NO FETCH failed\r\n")
                }
            };
            return session.push(response.as_bytes());
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
