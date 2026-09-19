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
            Pending::ImapAppend { tag },
            Response::Appended {
                result: Ok(uid), ..
            },
        ) => session.push(format!("{tag} OK [APPENDUID 1 {uid}] APPEND completed\r\n").as_bytes()),
        (Pending::ImapAppend { tag }, Response::Appended { result: Err(_), .. }) => {
            session.push(format!("{tag} NO APPEND failed\r\n").as_bytes())
        }
        (
            Pending::ReadForSelect {
                tag,
                mailbox,
                read_only,
            },
            Response::Read {
                result: Ok(snapshot),
                ..
            },
        ) => {
            session.selected_mailbox = Some(mailbox);
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
            if let Err(reason) = write_fetch_item(session, number, message, &attribute, addressing)?
            {
                return session.push(format!("{tag} NO {reason}\r\n").as_bytes());
            }
        }
    }
    session.push(format!("{tag} OK FETCH completed\r\n").as_bytes())
}

fn write_fetch_item(
    session: &mut Session,
    number: usize,
    message: &crate::store::Message,
    attribute: &imap::FetchAttribute,
    addressing: imap::Addressing,
) -> io::Result<Result<(), &'static str>> {
    let uid = match addressing {
        imap::Addressing::Sequence => String::new(),
        imap::Addressing::Uid => format!("UID {} ", message.uid),
    };
    match attribute {
        imap::FetchAttribute::Rfc822 => {
            session.push(
                format!(
                    "* {number} FETCH ({uid}RFC822 {{{}}}\r\n",
                    message.bytes.len()
                )
                .as_bytes(),
            )?;
            session.push(&message.bytes)?;
            session.push(b"\r\n)\r\n")?;
            Ok(Ok(()))
        }
        imap::FetchAttribute::Rfc822Size => {
            session.push(
                format!(
                    "* {number} FETCH ({uid}RFC822.SIZE {})\r\n",
                    message.bytes.len()
                )
                .as_bytes(),
            )?;
            Ok(Ok(()))
        }
        imap::FetchAttribute::BodyStructure => {
            let structure = crate::mime::tree::parse(&message.bytes)
                .and_then(|entity| crate::mime::bodystructure::encode(&entity, &message.bytes));
            match structure {
                Ok(structure) => {
                    session.push(
                        format!("* {number} FETCH ({uid}BODYSTRUCTURE {structure})\r\n").as_bytes(),
                    )?;
                    Ok(Ok(()))
                }
                Err(_) => Ok(Err("malformed MIME message")),
            }
        }
        imap::FetchAttribute::Body { .. } => {
            match crate::imap_fetch::select(&message.bytes, &attribute) {
                Ok(data) => {
                    session.push(
                        format!(
                            "* {number} FETCH ({uid}{} {{{}}}\r\n",
                            data.label,
                            data.bytes.len()
                        )
                        .as_bytes(),
                    )?;
                    session.push(data.bytes)?;
                    session.push(b"\r\n)\r\n")?;
                    Ok(Ok(()))
                }
                Err(_) => Ok(Err("invalid BODY section")),
            }
        }
        imap::FetchAttribute::Envelope => match crate::mime::envelope::encode(&message.bytes) {
            Ok(envelope) => {
                session
                    .push(format!("* {number} FETCH ({uid}ENVELOPE {envelope})\r\n").as_bytes())?;
                Ok(Ok(()))
            }
            Err(_) => Ok(Err("malformed message headers")),
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
