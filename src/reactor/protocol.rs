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
    while session.pending.is_none() && session.pending_submit.is_none() {
        let Some(line) = take_line(&mut session.input) else {
            break;
        };
        match session.kind {
            Kind::Smtp => smtp_line(session, line, slot, generation, worker, owners, request_id)?,
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
        | Request::Append { request_id, .. }
        | Request::Read { request_id, .. } => *request_id,
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

fn smtp_line(
    session: &mut Session,
    line: Vec<u8>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    if let SmtpState::Data(raw) = &mut session.smtp {
        if line == b".\r\n" {
            let bytes = std::mem::take(raw);
            session.smtp = SmtpState::Command(smtp::Transaction::default());
            let id = *request_id;
            *request_id = request_id.wrapping_add(1).max(1);
            submit(
                session,
                Request::Append {
                    request_id: id,
                    mailbox: "bob".into(),
                    bytes,
                },
                Pending::Append,
                slot,
                generation,
                worker,
                owners,
            );
        } else if raw.len().saturating_add(line.len()) > MAX_MESSAGE {
            session.smtp = SmtpState::Command(smtp::Transaction::default());
            session.push(b"552 message too large\r\n")?;
        } else {
            if line.starts_with(b"..") {
                raw.extend_from_slice(&line[1..]);
            } else {
                raw.extend_from_slice(&line);
            }
        }
        return Ok(());
    }
    let text = std::str::from_utf8(&line).unwrap_or("");
    let command = match smtp::parse(text) {
        Ok(command) => command,
        Err(_) => {
            session.push(b"500 syntax error\r\n")?;
            return Ok(());
        }
    };
    if command == smtp::Command::Quit {
        session.push(b"221 bye\r\n")?;
        return Ok(());
    }
    if matches!(command, smtp::Command::Ehlo(_)) {
        session.push(b"250-litterkeep.test\r\n250 SIZE 10485760\r\n")?;
        return Ok(());
    }
    let SmtpState::Command(tx) = &mut session.smtp else {
        unreachable!()
    };
    match tx.apply(&command) {
        Ok(true) => {
            session.smtp = SmtpState::Data(Vec::new());
            session.push(b"354 end with <CRLF>.<CRLF>\r\n")?;
        }
        Ok(false) => session.push(b"250 ok\r\n")?,
        Err(_) => session.push(b"503 bad sequence\r\n")?,
    }
    Ok(())
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
            session.push(b"* CAPABILITY IMAP4rev1 AUTH=PLAIN\r\n")?;
            session.push(format!("{} OK CAPABILITY completed\r\n", parsed.tag).as_bytes())?;
        }
        imap::Command::Login { username, password } => {
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
        imap::Command::Logout => {
            session.push(b"* BYE\r\n")?;
            session.push(format!("{} OK LOGOUT completed\r\n", parsed.tag).as_bytes())?;
        }
        imap::Command::Select | imap::Command::Examine => queue_authenticated_read(
            session,
            parsed.tag,
            Pending::ReadForSelect,
            slot,
            generation,
            worker,
            owners,
            request_id,
        )?,
        imap::Command::Fetch(message) => queue_authenticated_read(
            session,
            parsed.tag,
            |tag| Pending::ReadForFetch(message, tag),
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
    F: FnOnce(String) -> Pending,
{
    let Some(mailbox) = session.authenticated_account.clone() else {
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    submit(
        session,
        Request::Read {
            request_id: id,
            mailbox,
        },
        pending(tag),
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}
