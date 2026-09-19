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
    maildrops: &mut MaildropLeases,
) -> io::Result<()> {
    while let Ok(response) = worker.worker.try_response() {
        let id = completion_id::response_id(&response);
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
                .or_else(|| {
                    super::pop3_protocol::follow_up(
                        &pending,
                        &response,
                        next_request,
                        maildrops,
                        slot,
                        generation,
                    )
                })
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
        super::imap_idle::notify_flags(sessions, slot, &pending, &response)?;
        let Some(session) = sessions
            .get_mut(slot)
            .and_then(Option::as_mut)
            .map(SessionLease::get_mut)
        else {
            continue;
        };
        if super::pop3_completion::handles(&pending, &response) {
            super::pop3_completion::write(session, pending, response, maildrops, slot, generation)?;
            protocol::process_input(session, slot, generation, worker, owners, next_request)?;
            continue;
        }
        write_completion(session, pending, response)?;
        protocol::process_input(session, slot, generation, worker, owners, next_request)?;
    }
    Ok(())
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
            Pending::ManageSieveAuthenticate { account },
            Response::Authenticated { accepted, .. },
        ) => {
            let reply = session.managesieve.authenticated(accepted);
            if accepted {
                session.authenticated_account = Some(account);
            }
            managesieve_reply(session, reply)
        }
        (Pending::ManageSieveCommand, Response::SieveStored { result, .. }) => {
            sieve_result(session, result.map(|_| ()))
        }
        (Pending::ManageSieveCommand, Response::SieveMutated { result, .. }) => {
            sieve_result(session, result)
        }
        (Pending::ManageSieveCommand, Response::SieveSpace { result, .. }) => match result {
            Ok(true) => session.push(b"OK \"space available\"\r\n"),
            Ok(false) => session.push(b"NO (QUOTA) \"space unavailable\"\r\n"),
            Err(_) => session.push(b"NO \"storage failure\"\r\n"),
        },
        (Pending::ManageSieveCommand, Response::SieveScripts { result, .. }) => {
            sieve_list(session, result)
        }
        (Pending::ManageSieveCommand, Response::SieveSource { result, .. }) => {
            sieve_source(session, result)
        }
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
            Response::FolderSelected {
                result: Ok(resolved),
                ..
            },
        ) => {
            let snapshot = resolved.snapshot;
            session.selected_mailbox = Some(resolved.key);
            session.selected_exists = snapshot.messages.len();
            session.selected_recent = resolved.recent_uids.into_iter().collect();
            session.selected_read_only = read_only;
            let access = match read_only {
                true => "READ-ONLY",
                false => "READ-WRITE",
            };
            session.push(
                format!(
                    "* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n\
                     * {} EXISTS\r\n\
                     * {} RECENT\r\n\
                     * OK [UIDVALIDITY {}] UIDs valid\r\n\
                     * OK [UIDNEXT {}] Predicted next UID\r\n\
                     * OK [PERMANENTFLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)] Flags permitted\r\n\
                     {tag} OK [{access}] SELECT completed\r\n",
                    snapshot.messages.len(),
                    session.selected_recent.len(),
                    snapshot.uid_validity,
                    snapshot.uid_next,
                )
                .as_bytes(),
            )
        }
        (
            Pending::ReadForSelect { tag, .. },
            Response::FolderSelected {
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

fn managesieve_reply(session: &mut Session, reply: managesieve::Reply) -> io::Result<()> {
    match reply {
        managesieve::Reply::Text(bytes) => session.push(&bytes),
        _ => session.push(b"NO \"state failure\"\r\n"),
    }
}

fn sieve_result(session: &mut Session, result: io::Result<()>) -> io::Result<()> {
    match result {
        Ok(()) => session.push(b"OK \"completed\"\r\n"),
        Err(_) => session.push(b"NO \"operation failed\"\r\n"),
    }
}

fn sieve_list(
    session: &mut Session,
    result: io::Result<Vec<crate::store::ScriptInfo>>,
) -> io::Result<()> {
    match result {
        Ok(scripts) => {
            for script in scripts {
                let active = match script.active {
                    true => " ACTIVE",
                    false => "",
                };
                session.push(format!("\"{}\"{}\r\n", script.name, active).as_bytes())?;
            }
            session.push(b"OK \"list completed\"\r\n")
        }
        Err(_) => session.push(b"NO \"list failed\"\r\n"),
    }
}

fn sieve_source(session: &mut Session, result: io::Result<(u64, String)>) -> io::Result<()> {
    match result {
        Ok((_, source)) => session.push(
            format!(
                "{{{}}}\r\n{}\r\nOK \"get completed\"\r\n",
                source.len(),
                source
            )
            .as_bytes(),
        ),
        Err(_) => session.push(b"NO \"script not found\"\r\n"),
    }
}
