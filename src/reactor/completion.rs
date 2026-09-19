//! Обработка ответов worker-а и повторная постановка ограниченных запросов.
//! Этот слой сопоставляет completion с `(slot, generation)`. Поэтому поздний
//! ответ закрытой сессии не может стать ответом новому клиенту в том же slot.
use super::*;

/// Забирает все уже готовые ответы хранилища и продолжает остановленный ввод.
pub(super) fn complete(
    worker: &mut WakingStorageWorker,
    sessions: &mut [Option<SessionLease>],
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    next_request: &mut u64,
) -> io::Result<()> {
    while let Ok(response) = worker.worker.try_response() {
        let id = response_id(&response);
        let Some((slot, generation, pending)) = owners.remove(&id) else {
            continue;
        };
        super::imap_idle::notify_completion(sessions, &pending, &response)?;
        let Some(lease) = sessions.get_mut(slot).and_then(Option::as_mut) else {
            continue;
        };
        let session = lease.get_mut();
        if session.generation != generation {
            continue;
        }
        session.pending = None;
        if let Some((request, next_pending)) =
            super::imap_expunge::follow_up(&pending, &response, next_request)
                .or_else(|| super::flag_completion::follow_up(&pending, &response, next_request))
        {
            protocol::submit(
                session,
                request,
                next_pending,
                slot,
                generation,
                worker,
                owners,
            );
            continue;
        }
        write_completion(session, pending, response)?;
        protocol::process_input(session, slot, generation, worker, owners, next_request)?;
    }
    Ok(())
}

/// Возвращает идентификатор, выданный reactor-ом для каждого ответа worker-а.
fn response_id(response: &Response) -> u64 {
    match response {
        Response::Authenticated { request_id, .. }
        | Response::RecipientValidated { request_id, .. }
        | Response::Appended { request_id, .. }
        | Response::Read { request_id, .. }
        | Response::FlagsUpdated { request_id, .. }
        | Response::FolderCreated { request_id, .. }
        | Response::FolderMutated { request_id, .. }
        | Response::Folders { request_id, .. }
        | Response::FolderResolved { request_id, .. }
        | Response::FolderRead { request_id, .. } => *request_id,
        Response::SubscriptionSet { request_id, .. }
        | Response::Subscriptions { request_id, .. } => *request_id,
        Response::Expunged { request_id, .. } => *request_id,
    }
}

/// Превращает типизированный результат worker-а в уже согласованный ответ wire.
fn write_completion(session: &mut Session, pending: Pending, response: Response) -> io::Result<()> {
    match (pending, response) {
        (
            Pending::Authenticate { tag, account },
            Response::Authenticated { accepted: true, .. },
        ) => {
            session.authenticated_account = Some(account);
            session.push(format!("{tag} OK LOGIN completed\r\n").as_bytes())
        }
        (
            Pending::Authenticate { tag, .. },
            Response::Authenticated {
                accepted: false, ..
            },
        ) => session.push(format!("{tag} NO authentication failed\r\n").as_bytes()),
        (Pending::SmtpAuthenticate { account }, Response::Authenticated { accepted: true, .. }) => {
            session.authenticated_account = Some(account);
            session.push(b"235 2.7.0 authentication successful\r\n")
        }
        (
            Pending::SmtpAuthenticate { .. },
            Response::Authenticated {
                accepted: false, ..
            },
        ) => session.push(b"535 5.7.8 authentication credentials invalid\r\n"),
        (
            Pending::ValidateRecipient { address },
            Response::RecipientValidated { accepted: true, .. },
        ) => accept_recipient(session, address),
        (
            Pending::ValidateRecipient { .. },
            Response::RecipientValidated {
                accepted: false, ..
            },
        ) => session.push(b"550 unknown local recipient\r\n"),
        (Pending::Append { .. }, Response::Appended { result: Ok(_), .. }) => {
            session.push(b"250 queued\r\n")
        }
        (Pending::Append { .. }, Response::Appended { result: Err(_), .. }) => {
            session.push(b"451 storage failure\r\n")
        }
        (
            Pending::ImapAppend { tag, .. },
            Response::Appended {
                result: Ok(uid), ..
            },
        ) => session.push(format!("{tag} OK [APPENDUID 1 {uid}] APPEND completed\r\n").as_bytes()),
        (Pending::ImapAppend { tag, .. }, Response::Appended { result: Err(_), .. }) => {
            session.push(format!("{tag} NO APPEND failed\r\n").as_bytes())
        }
        (
            Pending::ReadForSelect { tag, read_only },
            Response::FolderRead {
                result: Ok(resolved),
                ..
            },
        ) => {
            let snapshot = resolved.snapshot;
            session.selected_mailbox = Some(resolved.key);
            session.selected_exists = snapshot.messages.len();
            session.selected_read_only = read_only;
            let access = match read_only {
                true => "READ-ONLY",
                false => "READ-WRITE",
            };
            session.push(
                format!(
                    "* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n\
                     * {} EXISTS\r\n\
                     * OK [UIDVALIDITY {}] UIDs valid\r\n\
                     * OK [UIDNEXT {}] Predicted next UID\r\n\
                     * OK [PERMANENTFLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)] Flags permitted\r\n\
                     {tag} OK [{access}] SELECT completed\r\n",
                    snapshot.messages.len(),
                    snapshot.uid_validity,
                    snapshot.uid_next,
                )
                .as_bytes(),
            )
        }
        (
            Pending::ReadForSelect { tag, .. },
            Response::FolderRead {
                result: Err(error), ..
            },
        ) => match error.kind() {
            io::ErrorKind::NotFound => {
                session.push(format!("{tag} NO mailbox does not exist\r\n").as_bytes())
            }
            _ => session.push(format!("{tag} NO SELECT failed\r\n").as_bytes()),
        },
        (
            Pending::ResolveForAppend {
                tag,
                literal_size,
                metadata,
            },
            Response::FolderResolved { result, .. },
        ) => imap_append::resolved(session, tag, literal_size, metadata, result),
        (
            Pending::ReadForFetch(addressing, sequence_set, attribute, tag),
            Response::Read {
                result: Ok(snapshot),
                ..
            },
        ) => write_fetch(
            session,
            addressing,
            sequence_set,
            attribute,
            tag,
            snapshot.messages,
        ),
        (_, Response::Read { .. }) => session.push(b"451 storage failure\r\n"),
        (pending @ Pending::WriteFlags { .. }, response @ Response::FlagsUpdated { .. }) => {
            super::flag_completion::write(session, pending, response)
        }
        (_, Response::FlagsUpdated { .. }) => session.push(b"451 storage failure\r\n"),
        (Pending::CreateFolder { tag }, Response::FolderCreated { result, .. }) => {
            imap_folders::created(session, tag, result)
        }
        (Pending::MutateFolder { tag, operation }, Response::FolderMutated { result, .. }) => {
            imap_folders::mutated(session, tag, operation, result)
        }
        (Pending::ListFolders { tag, pattern }, Response::Folders { result, .. }) => {
            imap_folders::listed(session, tag, pattern, result)
        }
        (
            Pending::SetSubscription { tag, subscribed },
            Response::SubscriptionSet { result, .. },
        ) => imap_subscriptions::set_completed(session, tag, subscribed, result),
        (Pending::ListSubscriptions { tag, pattern }, Response::Subscriptions { result, .. }) => {
            imap_subscriptions::listed(session, tag, pattern, result)
        }
        (
            Pending::Status {
                tag,
                name,
                attributes,
            },
            Response::FolderRead { result, .. },
        ) => imap_status::completed(session, tag, name, attributes, result),
        (
            Pending::Expunge {
                tag,
                sequence_numbers,
            },
            Response::Expunged { result, .. },
        ) => imap_expunge::completed(session, tag, sequence_numbers, result),
        _ => Ok(()),
    }
}

/// Завершает RCPT лишь после проверки policy у владельца account store.
fn accept_recipient(session: &mut Session, address: String) -> io::Result<()> {
    let SmtpState::Command(transaction) = &mut session.smtp else {
        return session.push(b"503 bad sequence\r\n");
    };
    match transaction.accept_recipient(address) {
        Ok(()) => session.push(b"250 ok\r\n"),
        Err(_) => session.push(b"503 bad sequence\r\n"),
    }
}

fn write_fetch(
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

/// Повторно предлагает worker-у запросы, которые ранее не поместились в канал.
pub(super) fn retry_submissions(
    worker: &mut WakingStorageWorker,
    sessions: &mut [Option<SessionLease>],
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
) -> io::Result<()> {
    for (slot, session) in sessions.iter_mut().enumerate() {
        let Some(lease) = session else { continue };
        let session = lease.get_mut();
        let Some((request, pending)) = session.pending_submit.take() else {
            continue;
        };
        let generation = session.generation;
        protocol::submit(session, request, pending, slot, generation, worker, owners);
    }
    Ok(())
}
