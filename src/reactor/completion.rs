//! Обработка ответов worker-а и повторная постановка ограниченных запросов.
//!
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
        let Some(lease) = sessions.get_mut(slot).and_then(Option::as_mut) else {
            continue;
        };
        let session = lease.get_mut();
        if session.generation != generation {
            continue;
        }
        session.pending = None;
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
        | Response::Read { request_id, .. } => *request_id,
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
        (Pending::Append, Response::Appended { result: Ok(_), .. }) => {
            session.push(b"250 queued\r\n")
        }
        (Pending::Append, Response::Appended { result: Err(_), .. }) => {
            session.push(b"451 storage failure\r\n")
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
                messages.len()
            )
            .as_bytes(),
        ),
        (
            Pending::ReadForFetch(number, attribute, tag),
            Response::Read {
                result: Ok(messages),
                ..
            },
        ) => write_fetch(session, number, attribute, tag, messages),
        (_, Response::Read { .. }) => session.push(b"451 storage failure\r\n"),
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

/// Записывает RFC822 literal или ответ об отсутствии запрошенного сообщения.
fn write_fetch(
    session: &mut Session,
    number: usize,
    attribute: imap::FetchAttribute,
    tag: String,
    messages: Vec<crate::store::Message>,
) -> io::Result<()> {
    let Some(message) = messages.get(number.saturating_sub(1)) else {
        return session.push(format!("{tag} NO no such message\r\n").as_bytes());
    };
    match attribute {
        imap::FetchAttribute::Rfc822 => {
            session.push(
                format!("* {number} FETCH (RFC822 {{{}}}\r\n", message.bytes.len()).as_bytes(),
            )?;
            session.push(&message.bytes)?;
            session.push(format!("\r\n)\r\n{tag} OK FETCH completed\r\n").as_bytes())
        }
        imap::FetchAttribute::Rfc822Size => session.push(
            format!(
                "* {number} FETCH (RFC822.SIZE {})\r\n{tag} OK FETCH completed\r\n",
                message.bytes.len()
            )
            .as_bytes(),
        ),
        imap::FetchAttribute::BodyStructure => {
            let structure = crate::mime::tree::parse(&message.bytes)
                .and_then(|entity| crate::mime::bodystructure::encode(&entity, &message.bytes));
            match structure {
                Ok(structure) => session.push(
                    format!(
                        "* {number} FETCH (BODYSTRUCTURE {structure})\r\n{tag} OK FETCH completed\r\n"
                    )
                    .as_bytes(),
                ),
                Err(_) => session.push(format!("{tag} NO malformed MIME message\r\n").as_bytes()),
            }
        }
        imap::FetchAttribute::Body { .. } => {
            match crate::imap_fetch::select(&message.bytes, &attribute) {
                Ok(data) => {
                    session.push(
                        format!(
                            "* {number} FETCH ({} {{{}}}\r\n",
                            data.label,
                            data.bytes.len()
                        )
                        .as_bytes(),
                    )?;
                    session.push(data.bytes)?;
                    session.push(format!("\r\n)\r\n{tag} OK FETCH completed\r\n").as_bytes())
                }
                Err(_) => session.push(format!("{tag} NO invalid BODY section\r\n").as_bytes()),
            }
        }
        imap::FetchAttribute::Envelope => match crate::mime::envelope::encode(&message.bytes) {
            Ok(envelope) => session.push(
                format!("* {number} FETCH (ENVELOPE {envelope})\r\n{tag} OK FETCH completed\r\n")
                    .as_bytes(),
            ),
            Err(_) => session.push(format!("{tag} NO malformed message headers\r\n").as_bytes()),
        },
    }
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
