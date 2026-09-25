//! Общий ограниченный dispatcher входа для четырёх сетевых протоколов.

use super::*;

fn take_line(input: &mut Vec<u8>) -> Option<Vec<u8>> {
    let end = input.windows(2).position(|pair| pair == b"\r\n")?;
    Some(input.drain(..end + 2).collect())
}

/// Выполняет команды до ожидания worker-а, literal или неполной строки.
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
        if session.imap_search_literal.is_some() {
            let progress = session
                .imap_search_literal
                .as_mut()
                .expect("SEARCH literal state checked")
                .consume(&mut session.input);
            match progress {
                imap_search_literal::Progress::Waiting => break,
                imap_search_literal::Progress::Continue => {
                    session.push(b"+ Ready for SEARCH literal\r\n")?;
                    break;
                }
                imap_search_literal::Progress::Complete(value) => {
                    session.imap_search_literal = None;
                    session.imap_search_literal_value = Some(value);
                    continue;
                }
                imap_search_literal::Progress::InvalidString => {
                    session.imap_search_literal = None;
                    session.termination = Some(session::SessionTermination::InvalidSearchLiteral);
                    break;
                }
                imap_search_literal::Progress::InvalidSyntax => {
                    session.imap_search_literal = None;
                    session.termination = Some(session::SessionTermination::InvalidSearchLiteral);
                    break;
                }
            }
        }
        if session.managesieve_literal.is_some() {
            match super::managesieve_protocol::literal(
                session, slot, generation, worker, owners, request_id,
            )? {
                true => continue,
                false => break,
            }
        }
        let Some(line) = take_line(&mut session.input) else {
            break;
        };
        if session.imap_authenticate.is_some() {
            super::imap_protocol::auth_continuation::handle(
                session, line, slot, generation, worker, owners, request_id,
            )?;
            continue;
        }
        if session.imap_idle.is_some() {
            imap_idle::continuation(session, &line)?;
            continue;
        }
        match session.kind {
            Kind::Smtp => super::smtp_protocol::smtp_line(
                session, line, slot, generation, worker, owners, request_id,
            )?,
            Kind::Imap => match session.imap_search_literal_value.take() {
                Some(values) => super::imap_protocol::line(
                    session,
                    line,
                    Some(values),
                    super::imap_line_context::LineContext {
                        slot,
                        generation,
                        worker,
                        owners,
                        request_id,
                    },
                )?,
                None => match imap_search_literal::start(&line) {
                    imap_search_literal::Start::NotSearchLiteral => super::imap_protocol::line(
                        session,
                        line,
                        None,
                        super::imap_line_context::LineContext {
                            slot,
                            generation,
                            worker,
                            owners,
                            request_id,
                        },
                    )?,
                    imap_search_literal::Start::Begin(receiver) => {
                        session.imap_search_literal = Some(receiver);
                        session.push(b"+ Ready for SEARCH literal\r\n")?;
                    }
                    imap_search_literal::Start::Reject { tag, reason } => {
                        session.push(format!("{tag} BAD {reason}\r\n").as_bytes())?;
                    }
                },
            },
            Kind::Pop3 => super::pop3_protocol::line(
                session, line, slot, generation, worker, owners, request_id,
            )?,
            Kind::ManageSieve => super::managesieve_protocol::line(
                session, line, slot, generation, worker, owners, request_id,
            )?,
        }
    }
    Ok(())
}

/// Передаёт bounded request единственному владельцу Store.
pub(super) fn submit(
    session: &mut Session,
    request: Request,
    pending: Pending,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
) {
    let request_id = submit_id::request_id(&request);
    match worker.worker.submit(request) {
        Ok(()) => {
            owners.insert(request_id, (slot, generation, pending.clone()));
            session.pending = Some(pending);
        }
        Err(std::sync::mpsc::TrySendError::Full(request)) => {
            session.pending_submit = Some((request, pending));
        }
        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
            let _ = session.push(b"451 storage unavailable\r\n");
        }
    }
}
