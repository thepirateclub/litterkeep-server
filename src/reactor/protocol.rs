use super::*;

fn take_line(input: &mut Vec<u8>) -> Option<Vec<u8>> {
    let end = input.windows(2).position(|pair| pair == b"\r\n")?;
    Some(input.drain(..end + 2).collect())
}

pub(super) fn process_input(
    session: &mut Session,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    while session.pending.is_none() {
        if session.pending_submit.is_some() {
            break;
        }
        if session.imap_literal.is_some() {
            match imap_append::consume(session, request_id)? {
                Some((request, pending)) => {
                    submit(session, request, pending, slot, generation, worker, owners);
                    continue;
                }
                None => break,
            }
        }
        if session.imap_idle.is_some() {
            let Some(line) = take_line(&mut session.input) else {
                break;
            };
            imap_idle::continuation(session, &line)?;
            continue;
        }
        let Some(line) = take_line(&mut session.input) else {
            break;
        };
        match session.kind {
            Kind::Smtp => super::smtp_protocol::smtp_line(
                session, line, slot, generation, worker, owners, request_id,
            )?,
            Kind::Imap => imap_line(session, line, slot, generation, worker, owners, request_id)?,
        }
    }
    Ok(())
}
pub(super) fn submit(
    session: &mut Session,
    request: Request,
    pending: Pending,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
) {
    let request_id = match &request {
        Request::Authenticate { request_id, .. }
        | Request::ValidateRecipient { request_id, .. }
        | Request::Append { request_id, .. }
        | Request::AppendMany { request_id, .. }
        | Request::Read { request_id, .. }
        | Request::UpdateFlags { request_id, .. }
        | Request::CreateFolder { request_id, .. }
        | Request::RenameFolder { request_id, .. }
        | Request::DeleteFolder { request_id, .. }
        | Request::ListFolders { request_id, .. }
        | Request::ResolveFolder { request_id, .. }
        | Request::ReadFolder { request_id, .. }
        | Request::SetSubscription { request_id, .. }
        | Request::ListSubscriptions { request_id, .. }
        | Request::Expunge { request_id, .. } => *request_id,
    };
    match worker.worker.submit(request) {
        Ok(()) => {
            owners.insert(request_id, (slot, generation, pending.clone()));
            session.pending = Some(pending);
        }
        Err(std::sync::mpsc::TrySendError::Full(request)) => {
            session.pending_submit = Some((request, pending))
        }
        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
            let _ = session.push(b"451 storage unavailable\r\n");
        }
    }
}
fn imap_line(
    session: &mut Session,
    line: Vec<u8>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let parsed = imap::parse(std::str::from_utf8(&line).unwrap_or(""));
    match parsed.command {
        imap::Command::Capability => {
            match (session.tls.is_some(), session.can_starttls()) {
                (true, _) => session.push(b"* CAPABILITY IMAP4rev1 IDLE AUTH=PLAIN\r\n")?,
                (false, true) => {
                    session.push(b"* CAPABILITY IMAP4rev1 IDLE STARTTLS LOGINDISABLED\r\n")?
                }
                (false, false) => session.push(b"* CAPABILITY IMAP4rev1 IDLE AUTH=PLAIN\r\n")?,
            }
            session.push(format!("{} OK CAPABILITY completed\r\n", parsed.tag).as_bytes())?;
        }
        imap::Command::StartTls => imap_starttls::begin(session, &parsed.tag)?,
        imap::Command::Noop => {
            session.push(format!("{} OK NOOP completed\r\n", parsed.tag).as_bytes())?
        }
        imap::Command::Login { username, password } => {
            if session.can_starttls() {
                return session.push(
                    format!("{} NO [PRIVACYREQUIRED] STARTTLS required\r\n", parsed.tag).as_bytes(),
                );
            }
            let id = *request_id;
            *request_id = request_id.wrapping_add(1).max(1);
            submit(
                session,
                Request::Authenticate {
                    request_id: id,
                    username: username.clone(),
                    password: password.into_bytes(),
                },
                Pending::Authenticate {
                    tag: parsed.tag,
                    account: username,
                },
                slot,
                generation,
                worker,
                owners,
            );
        }
        imap::Command::Append {
            mailbox,
            literal_size,
            flags,
            internal_date,
        } => imap_append::begin(
            session,
            parsed.tag,
            mailbox,
            literal_size,
            crate::store::AppendMetadata {
                flags,
                internal_date: internal_date.unwrap_or_else(crate::mail_date::MailDate::now),
            },
            slot,
            generation,
            worker,
            owners,
            request_id,
        )?,
        imap::Command::Logout => {
            session.push(b"* BYE\r\n")?;
            session.push(format!("{} OK LOGOUT completed\r\n", parsed.tag).as_bytes())?;
        }
        imap::Command::Idle => imap_idle::begin(session, parsed.tag)?,
        imap::Command::Create { mailbox } => imap_folders::create(
            session, parsed.tag, mailbox, slot, generation, worker, owners, request_id,
        )?,
        imap::Command::Delete { mailbox } => imap_folders::delete(
            session, parsed.tag, mailbox, slot, generation, worker, owners, request_id,
        )?,
        imap::Command::Rename {
            source,
            destination,
        } => imap_folders::rename(
            session,
            parsed.tag,
            source,
            destination,
            slot,
            generation,
            worker,
            owners,
            request_id,
        )?,
        imap::Command::List { pattern } => imap_folders::list(
            session, parsed.tag, pattern, slot, generation, worker, owners, request_id,
        )?,
        imap::Command::Subscribe { mailbox } => imap_subscriptions::set(
            session, parsed.tag, mailbox, true, slot, generation, worker, owners, request_id,
        )?,
        imap::Command::Unsubscribe { mailbox } => imap_subscriptions::set(
            session, parsed.tag, mailbox, false, slot, generation, worker, owners, request_id,
        )?,
        imap::Command::Lsub { pattern } => imap_subscriptions::list(
            session, parsed.tag, pattern, slot, generation, worker, owners, request_id,
        )?,
        imap::Command::Status {
            mailbox,
            attributes,
        } => imap_status::queue(
            session, parsed.tag, mailbox, attributes, slot, generation, worker, owners, request_id,
        )?,
        imap::Command::Expunge => imap_expunge::queue(
            session, parsed.tag, slot, generation, worker, owners, request_id, false,
        )?,
        imap::Command::Close => imap_expunge::queue(
            session, parsed.tag, slot, generation, worker, owners, request_id, true,
        )?,
        imap::Command::Check => imap_check::execute(session, parsed.tag)?,
        imap::Command::Select { mailbox } => imap_select::queue(
            session, parsed.tag, mailbox, false, slot, generation, worker, owners, request_id,
        )?,
        imap::Command::Examine { mailbox } => imap_select::queue(
            session, parsed.tag, mailbox, true, slot, generation, worker, owners, request_id,
        )?,
        imap::Command::Fetch {
            addressing,
            sequence_set,
            attribute,
        } => imap_selected::queue_authenticated_read(
            session,
            parsed.tag,
            |tag, _| Pending::ReadForFetch(addressing, sequence_set, attribute, tag),
            slot,
            generation,
            worker,
            owners,
            request_id,
        )?,
        imap::Command::Store {
            addressing,
            sequence_set,
            mutation,
            silent,
            flags,
        } => imap_selected::queue_authenticated_read(
            session,
            parsed.tag,
            |tag, mailbox| Pending::ReadForStore {
                addressing,
                sequence_set,
                mutation,
                flags,
                silent,
                tag,
                mailbox,
            },
            slot,
            generation,
            worker,
            owners,
            request_id,
        )?,
        imap::Command::Search { addressing, key } => imap_selected::queue_authenticated_read(
            session,
            parsed.tag,
            |tag, _| Pending::ReadForSearch {
                addressing,
                key,
                tag,
            },
            slot,
            generation,
            worker,
            owners,
            request_id,
        )?,
        imap::Command::Unsupported => session.push(format!("{} BAD\r\n", parsed.tag).as_bytes())?,
    }
    Ok(())
}
