//! Маршрутизация типизированных IMAP-команд в state-machine reactor-а.

use super::*;

#[path = "imap_enable.rs"]
mod imap_enable;
#[path = "imap_list_dispatch.rs"]
mod imap_list_dispatch;
#[path = "imap_search_error.rs"]
pub(super) mod imap_search_error;
#[path = "imap_search_response.rs"]
pub(super) mod imap_search_response;

#[path = "imap_authenticate.rs"]
pub(super) mod authenticate;

/// Обрабатывает одну завершённую командную строку без блокирующего I/O.
pub(super) fn line(
    session: &mut Session,
    line: Vec<u8>,
    literal: Option<String>,
    context: super::imap_line_context::LineContext<'_>,
) -> io::Result<()> {
    let super::imap_line_context::LineContext {
        slot,
        generation,
        worker,
        owners,
        request_id,
    } = context;
    let command = std::str::from_utf8(&line).unwrap_or("");
    let parsed = match literal {
        Some(value) => imap::parse_search_literal(command, &value),
        None => imap::parse(command),
    };
    match parsed.command {
        imap::Command::Capability => imap_capability::write(session, &parsed.tag)?,
        imap::Command::Namespace => imap_namespace::write(session, &parsed.tag)?,
        imap::Command::Enable { capabilities } => {
            imap_enable::execute(session, &parsed.tag, capabilities)?
        }
        imap::Command::StartTls => imap_starttls::begin(session, &parsed.tag)?,
        imap::Command::Noop => {
            session.push(format!("{} OK NOOP completed\r\n", parsed.tag).as_bytes())?
        }
        imap::Command::Login { username, password } => authenticate::login(
            session,
            parsed.tag,
            username,
            password.into_bytes(),
            slot,
            generation,
            worker,
            owners,
            request_id,
        )?,
        imap::Command::Authenticate {
            mechanism,
            initial_response,
        } => authenticate::begin(
            session,
            parsed.tag,
            mechanism,
            initial_response,
            slot,
            generation,
            worker,
            owners,
            request_id,
        )?,
        imap::Command::Append {
            mailbox,
            literal_size,
            flags,
            keywords,
            internal_date,
        } => imap_append::begin(
            session,
            parsed.tag,
            mailbox,
            literal_size,
            crate::store::AppendMetadata {
                flags,
                keywords,
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
        imap::Command::Create {
            mailbox,
            special_use,
        } => imap_folders::create(
            session,
            parsed.tag,
            mailbox,
            special_use,
            slot,
            generation,
            worker,
            owners,
            request_id,
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
        imap::Command::List { pattern, returns } => {
            imap_list_dispatch::Dispatch::new(slot, generation, worker, owners, request_id)
                .basic(session, parsed.tag, pattern, returns)?
        }
        imap::Command::ListSelection {
            pattern,
            selection,
            returns,
        } => imap_list_dispatch::Dispatch::new(slot, generation, worker, owners, request_id)
            .selected(session, parsed.tag, pattern, selection, returns)?,
        imap::Command::Subscribe { mailbox } => {
            imap_list_dispatch::Dispatch::new(slot, generation, worker, owners, request_id)
                .subscription(session, parsed.tag, mailbox, true)?
        }
        imap::Command::Unsubscribe { mailbox } => {
            imap_list_dispatch::Dispatch::new(slot, generation, worker, owners, request_id)
                .subscription(session, parsed.tag, mailbox, false)?
        }
        imap::Command::Lsub { pattern } => {
            imap_list_dispatch::Dispatch::new(slot, generation, worker, owners, request_id)
                .lsub(session, parsed.tag, pattern)?
        }
        imap::Command::Status {
            mailbox,
            attributes,
        } => imap_status::queue(
            session, parsed.tag, mailbox, attributes, slot, generation, worker, owners, request_id,
        )?,
        imap::Command::Expunge => imap_expunge::queue(
            session, parsed.tag, slot, generation, worker, owners, request_id, false, None,
        )?,
        imap::Command::Close => imap_expunge::queue(
            session, parsed.tag, slot, generation, worker, owners, request_id, true, None,
        )?,
        imap::Command::Unselect => imap_unselect::execute(session, parsed.tag)?,
        imap::Command::UidExpunge { sequence_set } => imap_expunge::queue(
            session,
            parsed.tag,
            slot,
            generation,
            worker,
            owners,
            request_id,
            false,
            Some(sequence_set),
        )?,
        imap::Command::Check => imap_check::execute(session, parsed.tag)?,
        imap::Command::Copy {
            transfer,
            addressing,
            sequence_set,
            destination,
        } => imap_copy::queue(
            session,
            parsed.tag,
            transfer,
            addressing,
            sequence_set,
            destination,
            slot,
            generation,
            worker,
            owners,
            request_id,
        )?,
        imap::Command::Select { mailbox } => imap_select::queue(
            session, parsed.tag, mailbox, false, slot, generation, worker, owners, request_id,
        )?,
        imap::Command::Examine { mailbox } => imap_select::queue(
            session, parsed.tag, mailbox, true, slot, generation, worker, owners, request_id,
        )?,
        imap::Command::Fetch {
            addressing,
            sequence_set,
            attributes,
        } => {
            let read_only = session.selected_read_only;
            let saved_uids = session.saved_search_uids.clone();
            imap_selected::queue_authenticated_read(
                session,
                parsed.tag,
                |tag, mailbox| Pending::ReadForFetch {
                    addressing,
                    sequence_set,
                    attributes,
                    tag,
                    mailbox,
                    read_only,
                    saved_uids,
                },
                slot,
                generation,
                worker,
                owners,
                request_id,
            )?
        }
        imap::Command::Store {
            addressing,
            sequence_set,
            mutation,
            silent,
            flags,
            keywords,
        } => {
            let saved_uids = session.saved_search_uids.clone();
            imap_selected::queue_authenticated_read(
                session,
                parsed.tag,
                |tag, mailbox| Pending::ReadForStore {
                    addressing,
                    sequence_set,
                    mutation,
                    flags,
                    keywords,
                    silent,
                    tag,
                    mailbox,
                    saved_uids,
                },
                slot,
                generation,
                worker,
                owners,
                request_id,
            )?
        }
        imap::Command::Search {
            charset: imap::search_charset::Charset::Unsupported,
            result,
            ..
        } => imap_search_error::unsupported_charset(session, &parsed.tag, result.saves_result())?,
        imap::Command::Search {
            addressing,
            key,
            charset,
            result,
        } => imap_selected::queue_authenticated_read(
            session,
            parsed.tag,
            |tag, _| Pending::ReadForSearch {
                addressing,
                key,
                charset,
                result,
                tag,
            },
            slot,
            generation,
            worker,
            owners,
            request_id,
        )?,
        imap::Command::Unsupported => {
            session.push(format!("{} BAD invalid command syntax\r\n", parsed.tag).as_bytes())?
        }
    }
    Ok(())
}

/// Продолжает SASL exchange, уже отделённый от обычной командной grammar.
pub(super) fn authenticate_continuation(
    session: &mut Session,
    line: Vec<u8>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    authenticate::continuation(session, &line, slot, generation, worker, owners, request_id)
}
