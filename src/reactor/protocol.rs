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
            match consume_imap_literal(session, request_id)? {
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
        | Request::UpdateFlags { request_id, .. } => *request_id,
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
        imap::Command::StartTls => imap_starttls(session, &parsed.tag)?,
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
        } => begin_imap_append(session, parsed.tag, mailbox, literal_size)?,
        imap::Command::Logout => {
            session.push(b"* BYE\r\n")?;
            session.push(format!("{} OK LOGOUT completed\r\n", parsed.tag).as_bytes())?;
        }
        imap::Command::Idle => imap_idle::begin(session, parsed.tag)?,
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
        } => queue_authenticated_read(
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
        } => queue_authenticated_read(
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
        imap::Command::Unsupported => session.push(format!("{} BAD\r\n", parsed.tag).as_bytes())?,
    }
    Ok(())
}

/// Резервирует bounded literal только после TLS и успешной аутентификации.
fn begin_imap_append(
    session: &mut Session,
    tag: String,
    mailbox: imap::MailboxName,
    literal_size: usize,
) -> io::Result<()> {
    let Some(account) = session.authenticated_account.clone() else {
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    if let imap::MailboxName::Named(_) = mailbox {
        return session.push(format!("{tag} NO [TRYCREATE] mailbox does not exist\r\n").as_bytes());
    }
    if literal_size > MAX_MESSAGE {
        return session.push(format!("{tag} NO message exceeds limit\r\n").as_bytes());
    }
    session.imap_literal = Some(LiteralReceiver::new(tag, account, literal_size));
    session.push(b"+ Ready for literal data\r\n")
}

/// Делает один неблокирующий шаг literal receiver и формирует durable request.
fn consume_imap_literal(
    session: &mut Session,
    request_id: &mut u64,
) -> io::Result<Option<(Request, Pending)>> {
    let progress = session
        .imap_literal
        .as_mut()
        .expect("literal receiver checked")
        .consume(&mut session.input);
    match progress {
        imap_literal::LiteralProgress::Waiting => Ok(None),
        imap_literal::LiteralProgress::InvalidTerminator(tag) => {
            session.imap_literal = None;
            session.input.clear();
            session.push(format!("{tag} BAD invalid literal terminator\r\n").as_bytes())?;
            Ok(None)
        }
        imap_literal::LiteralProgress::Complete(completed) => {
            session.imap_literal = None;
            let id = *request_id;
            *request_id = request_id.wrapping_add(1).max(1);
            Ok(Some((
                Request::Append {
                    request_id: id,
                    mailbox: completed.mailbox.clone(),
                    bytes: completed.bytes,
                },
                Pending::ImapAppend {
                    tag: completed.tag,
                    mailbox: completed.mailbox,
                },
            )))
        }
    }
}

/// Запускает IMAP transport upgrade до аутентификации и mailbox state.
fn imap_starttls(session: &mut Session, tag: &str) -> io::Result<()> {
    match (
        session.can_starttls(),
        session.authenticated_account.is_none(),
    ) {
        (true, true) => {
            session.push(format!("{tag} OK Begin TLS negotiation now\r\n").as_bytes())?;
            session.request_starttls();
            Ok(())
        }
        (true, false) => session.push(format!("{tag} BAD invalid state\r\n").as_bytes()),
        (false, _) => session.push(format!("{tag} BAD STARTTLS unavailable\r\n").as_bytes()),
    }
}

/// Ставит чтение только после worker-подтверждённой аутентификации сессии.
fn queue_authenticated_read<F>(
    session: &mut Session,
    tag: String,
    pending: F,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()>
where
    F: FnOnce(String, String) -> Pending,
{
    let Some(mailbox) = session.selected_mailbox.clone() else {
        return session.push(format!("{tag} NO select mailbox first\r\n").as_bytes());
    };
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    submit(
        session,
        Request::Read {
            request_id: id,
            mailbox: mailbox.clone(),
        },
        pending(tag, mailbox),
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}
