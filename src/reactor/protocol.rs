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

fn submit(
    session: &mut Session,
    request: Request,
    pending: Pending,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
) {
    let request_id = match &request {
        Request::Append { request_id, .. } | Request::Read { request_id, .. } => *request_id,
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
    let text = std::str::from_utf8(&line).unwrap_or("");
    let mut parts = text.split_whitespace();
    let tag = parts.next().unwrap_or("*");
    match parts.next().unwrap_or("").to_ascii_uppercase().as_str() {
        "CAPABILITY" => {
            session.push(b"* CAPABILITY IMAP4rev1 AUTH=PLAIN\r\n")?;
            session.push(format!("{tag} OK CAPABILITY completed\r\n").as_bytes())?;
        }
        "LOGIN" => session.push(format!("{tag} OK LOGIN completed\r\n").as_bytes())?,
        "LOGOUT" => {
            session.push(b"* BYE\r\n")?;
            session.push(format!("{tag} OK LOGOUT completed\r\n").as_bytes())?;
        }
        "SELECT" | "EXAMINE" => queue_read(
            session,
            Pending::ReadForSelect(tag.into()),
            slot,
            generation,
            worker,
            owners,
            request_id,
        ),
        "FETCH" => {
            let message = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
            queue_read(
                session,
                Pending::ReadForFetch(message, tag.into()),
                slot,
                generation,
                worker,
                owners,
                request_id,
            );
        }
        _ => session.push(format!("{tag} BAD\r\n").as_bytes())?,
    }
    Ok(())
}

fn queue_read(
    session: &mut Session,
    pending: Pending,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) {
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    submit(
        session,
        Request::Read {
            request_id: id,
            mailbox: "bob".into(),
        },
        pending,
        slot,
        generation,
        worker,
        owners,
    );
}

pub(super) fn complete(
    worker: &mut WakingStorageWorker,
    sessions: &mut [Option<Session>],
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    next_request: &mut u64,
) -> io::Result<()> {
    while let Ok(response) = worker.worker.try_response() {
        let id = match &response {
            Response::Appended { request_id, .. } | Response::Read { request_id, .. } => {
                *request_id
            }
        };
        let Some((slot, generation, pending)) = owners.remove(&id) else {
            continue;
        };
        let Some(session) = sessions.get_mut(slot).and_then(Option::as_mut) else {
            continue;
        };
        if session.generation != generation {
            continue;
        }
        session.pending = None;
        match (pending, response) {
            (Pending::Append, Response::Appended { result: Ok(_), .. }) => {
                session.push(b"250 queued\r\n")?
            }
            (Pending::Append, Response::Appended { result: Err(_), .. }) => {
                session.push(b"451 storage failure\r\n")?
            }
            (
                Pending::ReadForSelect(tag),
                Response::Read {
                    result: Ok(messages),
                    ..
                },
            ) => session.push(
                format!(
                    "* {} EXISTS\r\n* OK [READ-ONLY]\r\n{tag} OK SELECT completed\r\n",
                    messages.len(),
                )
                .as_bytes(),
            )?,
            (
                Pending::ReadForFetch(n, tag),
                Response::Read {
                    result: Ok(messages),
                    ..
                },
            ) => {
                if let Some(message) = messages.get(n.saturating_sub(1)) {
                    session.push(
                        format!("* {n} FETCH (RFC822 {{{}}}\r\n", message.bytes.len()).as_bytes(),
                    )?;
                    session.push(&message.bytes)?;
                    session.push(format!("\r\n)\r\n{tag} OK FETCH completed\r\n").as_bytes())?;
                } else {
                    session.push(format!("{tag} NO no such message\r\n").as_bytes())?
                }
            }
            (_, Response::Read { .. }) => session.push(b"451 storage failure\r\n")?,
            _ => {}
        }
        process_input(session, slot, generation, worker, owners, next_request)?;
    }
    Ok(())
}

pub(super) fn retry_submissions(
    worker: &mut WakingStorageWorker,
    sessions: &mut [Option<Session>],
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
) -> io::Result<()> {
    for (slot, session) in sessions.iter_mut().enumerate() {
        let Some(session) = session else { continue };
        let Some((request, pending)) = session.pending_submit.take() else {
            continue;
        };
        let generation = session.generation;
        submit(session, request, pending, slot, generation, worker, owners);
    }
    Ok(())
}
