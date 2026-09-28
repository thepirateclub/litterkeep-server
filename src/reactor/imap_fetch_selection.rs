//! Вычисление ранга matching messages для RFC 9394 UID FETCH PARTIAL.

use super::{imap, imap_fetch_completion};
use std::collections::BTreeSet;
use std::ops::Range;

/// Возвращает позиции внутри уже отфильтрованного FETCH result sequence.
pub(super) fn positions(
    addressing: imap::Addressing,
    sequence_set: &imap::sequence_set::MessageSet,
    partial: Option<&imap::partial_range::PartialRange>,
    messages: &[crate::store::Message],
    saved_uids: &BTreeSet<u64>,
) -> Range<usize> {
    let maximum = match addressing {
        imap::Addressing::Sequence => messages.len() as u64,
        imap::Addressing::Uid => messages.last().map(|message| message.uid).unwrap_or(0),
    };
    let count = messages
        .iter()
        .enumerate()
        .filter(|(offset, message)| {
            let sequence = offset.saturating_add(1) as u64;
            let candidate = match addressing {
                imap::Addressing::Sequence => sequence,
                imap::Addressing::Uid => message.uid,
            };
            imap_fetch_completion::matches(
                sequence_set,
                candidate,
                message.uid,
                maximum,
                saved_uids,
            )
        })
        .count();
    match partial {
        Some(range) => range.positions(count),
        None => 0..count,
    }
}

#[cfg(test)]
#[path = "imap_fetch_selection_tests.rs"]
mod tests;
