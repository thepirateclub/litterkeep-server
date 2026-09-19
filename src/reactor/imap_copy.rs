//! Двухфазная IMAP COPY: snapshot source, resolve destination, durable commit.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn queue(
    session: &mut Session,
    tag: String,
    transfer: imap::Transfer,
    addressing: imap::Addressing,
    sequence_set: imap::sequence_set::SequenceSet,
    destination: imap::MailboxName,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let Some(account) = session.authenticated_account.clone() else {
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    let Some(source) = session.selected_mailbox.clone() else {
        return session.push(format!("{tag} NO select mailbox first\r\n").as_bytes());
    };
    if session.selected_read_only {
        return session.push(format!("{tag} NO [READ-ONLY] COPY denied\r\n").as_bytes());
    }
    let destination_name = wire_name(destination);
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    protocol::submit(
        session,
        Request::Read {
            request_id: id,
            mailbox: source.clone(),
        },
        Pending::ReadForCopy {
            tag,
            transfer,
            addressing,
            sequence_set,
            account,
            source,
            destination_name,
        },
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

/// Продолжает COPY только с UID из единого snapshot source mailbox.
pub(super) fn follow_up(
    pending: &Pending,
    response: &Response,
    next_request: &mut u64,
) -> Option<(Request, Pending)> {
    match (pending, response) {
        (
            Pending::ReadForCopy {
                tag,
                transfer,
                addressing,
                sequence_set,
                account,
                source,
                destination_name,
            },
            Response::Read {
                result: Ok(snapshot),
                ..
            },
        ) => {
            let source_uids = selected_uids(*addressing, sequence_set, &snapshot.messages);
            let source_sequences = selected_sequences(&source_uids, &snapshot.messages);
            let request_id = next(next_request);
            Some((
                Request::ResolveFolder {
                    request_id,
                    account: account.clone(),
                    name: destination_name.clone(),
                },
                Pending::ResolveCopyDestination {
                    tag: tag.clone(),
                    transfer: *transfer,
                    source: source.clone(),
                    source_uids,
                    source_sequences,
                },
            ))
        }
        (
            Pending::ResolveCopyDestination {
                tag,
                transfer,
                source,
                source_uids,
                source_sequences,
            },
            Response::FolderResolved {
                result: Ok(destination),
                ..
            },
        ) => Some((
            request(
                *transfer,
                next(next_request),
                source.clone(),
                destination.clone(),
                source_uids.clone(),
            ),
            Pending::Copy {
                tag: tag.clone(),
                transfer: *transfer,
                source_uids: source_uids.clone(),
                source_sequences: source_sequences.clone(),
            },
        )),
        _ => None,
    }
}

pub(super) fn completed(
    session: &mut Session,
    tag: String,
    transfer: imap::Transfer,
    source_uids: Vec<u64>,
    source_sequences: Vec<usize>,
    result: io::Result<Vec<u64>>,
) -> io::Result<()> {
    if source_uids.is_empty() {
        return session.push(format!("{tag} OK {} completed\r\n", verb(transfer)).as_bytes());
    }
    match result {
        Ok(destination_uids) => {
            let source = sequence_set(&source_uids);
            let destination = sequence_set(&destination_uids);
            if matches!(transfer, imap::Transfer::Move) {
                for number in source_sequences {
                    session.push(format!("* {number} EXPUNGE\r\n").as_bytes())?;
                }
                session.selected_exists = session.selected_exists.saturating_sub(source_uids.len());
            }
            session.push(
                format!(
                    "{tag} OK [COPYUID 1 {source} {destination}] {} completed\r\n",
                    verb(transfer)
                )
                .as_bytes(),
            )
        }
        Err(_) => session.push(format!("{tag} NO {} failed\r\n", verb(transfer)).as_bytes()),
    }
}

fn request(
    transfer: imap::Transfer,
    request_id: u64,
    source: String,
    destination: String,
    uids: Vec<u64>,
) -> Request {
    match transfer {
        imap::Transfer::Copy => Request::Copy {
            request_id,
            source,
            destination,
            uids,
        },
        imap::Transfer::Move => Request::Move {
            request_id,
            source,
            destination,
            uids,
        },
    }
}

fn verb(transfer: imap::Transfer) -> &'static str {
    match transfer {
        imap::Transfer::Copy => "COPY",
        imap::Transfer::Move => "MOVE",
    }
}

fn selected_uids(
    addressing: imap::Addressing,
    sequence_set: &imap::sequence_set::SequenceSet,
    messages: &[crate::store::Message],
) -> Vec<u64> {
    let maximum = match addressing {
        imap::Addressing::Sequence => messages.len() as u64,
        imap::Addressing::Uid => messages.last().map(|message| message.uid).unwrap_or(0),
    };
    let resolved = sequence_set.resolve(maximum);
    messages
        .iter()
        .enumerate()
        .filter_map(|(offset, message)| {
            let candidate = match addressing {
                imap::Addressing::Sequence => offset.saturating_add(1) as u64,
                imap::Addressing::Uid => message.uid,
            };
            resolved.contains(candidate).then_some(message.uid)
        })
        .collect()
}

fn selected_sequences(uids: &[u64], messages: &[crate::store::Message]) -> Vec<usize> {
    let mut removed = 0_usize;
    let mut sequences = Vec::with_capacity(uids.len());
    for (offset, message) in messages.iter().enumerate() {
        if uids.contains(&message.uid) {
            sequences.push(offset.saturating_add(1).saturating_sub(removed));
            removed = removed.saturating_add(1);
        }
    }
    sequences
}

fn wire_name(mailbox: imap::MailboxName) -> String {
    match mailbox {
        imap::MailboxName::Inbox => "INBOX".to_owned(),
        imap::MailboxName::Named(name) => name,
    }
}

fn next(request_id: &mut u64) -> u64 {
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    id
}

fn sequence_set(uids: &[u64]) -> String {
    uids.iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}
