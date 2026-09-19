//! Запись FETCH completion из готового snapshot selected mailbox.

use super::*;

pub(super) fn write(
    session: &mut Session,
    addressing: imap::Addressing,
    sequence_set: imap::sequence_set::SequenceSet,
    attribute: imap::FetchAttribute,
    tag: String,
    messages: Vec<crate::store::Message>,
) -> io::Result<()> {
    let maximum = match addressing {
        imap::Addressing::Sequence => messages.len() as u64,
        imap::Addressing::Uid => messages.last().map(|message| message.uid).unwrap_or(0),
    };
    let resolved = sequence_set.resolve(maximum);
    for (offset, message) in messages.iter().enumerate() {
        let number = offset.saturating_add(1);
        let candidate = match addressing {
            imap::Addressing::Sequence => number as u64,
            imap::Addressing::Uid => message.uid,
        };
        if resolved.contains(candidate) {
            if let Err(reason) =
                imap_fetch_response::write(session, number, message, &attribute, addressing)?
            {
                return session.push(format!("{tag} NO {reason}\r\n").as_bytes());
            }
        }
    }
    session.push(format!("{tag} OK FETCH completed\r\n").as_bytes())
}
