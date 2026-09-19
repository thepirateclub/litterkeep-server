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
        if !matches!(sessions.get(slot).and_then(Option::as_ref).map(|lease| lease.get().generation), Some(value) if value == generation)
        {
            continue;
        }
        if let Some(session) = sessions
            .get_mut(slot)
            .and_then(Option::as_mut)
            .map(SessionLease::get_mut)
        {
            session.pending = None;
        }
        if let Some((request, next_pending)) =
            super::imap_expunge::follow_up(&pending, &response, next_request)
                .or_else(|| super::flag_completion::follow_up(&pending, &response, next_request))
                .or_else(|| super::imap_copy::follow_up(&pending, &response, next_request))
        {
            let Some(session) = sessions
                .get_mut(slot)
                .and_then(Option::as_mut)
                .map(SessionLease::get_mut)
            else {
                continue;
            };
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
        super::imap_idle::notify_expunge(sessions, slot, &pending, &response)?;
        let Some(session) = sessions
            .get_mut(slot)
            .and_then(Option::as_mut)
            .map(SessionLease::get_mut)
        else {
            continue;
        };
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
        Response::Copied { request_id, .. } | Response::Moved { request_id, .. } => *request_id,
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
        ) => smtp_completion::accept_recipient(session, address),
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
        ) => imap_fetch_completion::write(
            session,
            addressing,
            sequence_set,
            attribute,
            tag,
            snapshot.messages,
        ),
        (
            Pending::ReadForSearch {
                addressing,
                key,
                tag,
            },
            Response::Read {
                result: Ok(snapshot),
                ..
            },
        ) => imap_search::completed(session, addressing, key, tag, snapshot.messages),
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
        (
            Pending::ListFolders {
                tag,
                pattern,
                special_use_only,
            },
            Response::Folders { result, .. },
        ) => imap_list::listed(session, tag, pattern, special_use_only, result),
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
                mailbox: _,
                sequence_numbers,
                close,
            },
            Response::Expunged { result, .. },
        ) => imap_expunge::completed(session, tag, sequence_numbers, result, close),
        (
            Pending::Copy {
                tag,
                transfer,
                source_uids,
                source_sequences,
            },
            Response::Copied { result, .. },
        ) => imap_copy::completed(
            session,
            tag,
            transfer,
            source_uids,
            source_sequences,
            result,
        ),
        (
            Pending::Copy {
                tag,
                transfer,
                source_uids,
                source_sequences,
            },
            Response::Moved { result, .. },
        ) => imap_copy::completed(
            session,
            tag,
            transfer,
            source_uids,
            source_sequences,
            result,
        ),
        (
            Pending::ResolveCopyDestination { tag, .. },
            Response::FolderResolved { result: Err(_), .. },
        ) => session
            .push(format!("{tag} NO [TRYCREATE] COPY destination unavailable\r\n").as_bytes()),
        _ => Ok(()),
    }
}
