//! Асинхронные SUBSCRIBE, UNSUBSCRIBE и LSUB через владельца store.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn set(
    session: &mut Session,
    tag: String,
    mailbox: imap::MailboxName,
    subscribed: bool,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let Some(account) = session.authenticated_account.clone() else {
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    let name = wire_name(mailbox);
    submit(
        session,
        Request::SetSubscription {
            request_id: *request_id,
            account,
            name,
            subscribed,
        },
        Pending::SetSubscription { tag, subscribed },
        slot,
        generation,
        worker,
        owners,
        request_id,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn list(
    session: &mut Session,
    tag: String,
    pattern: imap::folder::ListPattern,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
    response: SubscriptionResponse,
) -> io::Result<()> {
    let Some(account) = session.authenticated_account.clone() else {
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    submit(
        session,
        Request::ListSubscriptions {
            request_id: *request_id,
            account,
        },
        Pending::ListSubscriptions {
            tag,
            pattern,
            response,
        },
        slot,
        generation,
        worker,
        owners,
        request_id,
    );
    Ok(())
}

pub(super) fn set_completed(
    session: &mut Session,
    tag: String,
    subscribed: bool,
    result: io::Result<()>,
) -> io::Result<()> {
    let verb = match subscribed {
        true => "SUBSCRIBE",
        false => "UNSUBSCRIBE",
    };
    match result {
        Ok(()) => session.push(format!("{tag} OK {verb} completed\r\n").as_bytes()),
        Err(_) => session.push(format!("{tag} NO {verb} failed\r\n").as_bytes()),
    }
}

pub(super) fn listed(
    session: &mut Session,
    tag: String,
    pattern: imap::folder::ListPattern,
    response: SubscriptionResponse,
    result: io::Result<crate::store::SubscriptionSnapshot>,
) -> io::Result<()> {
    let snapshot = match result {
        Ok(snapshot) => snapshot,
        Err(_) => {
            return match response {
                SubscriptionResponse::Lsub => {
                    session.push(format!("{tag} NO LSUB failed\r\n").as_bytes())
                }
                SubscriptionResponse::List { .. } => {
                    session.push(format!("{tag} NO LIST failed\r\n").as_bytes())
                }
            };
        }
    };
    match response {
        SubscriptionResponse::Lsub => listed_lsub(session, tag, pattern, snapshot.subscriptions),
        SubscriptionResponse::List { selection, returns } => {
            imap_list_selection::listed(session, tag, pattern, selection, returns, snapshot)
        }
    }
}

fn listed_lsub(
    session: &mut Session,
    tag: String,
    pattern: imap::folder::ListPattern,
    subscriptions: Vec<crate::store::SubscribedFolder>,
) -> io::Result<()> {
    for folder in subscriptions
        .iter()
        .filter(|folder| imap_list::matches_pattern(&folder.name, &pattern))
    {
        let attributes = lsub_attributes(folder);
        let name = imap_folders::quote(&folder.name);
        session.push(format!("* LSUB ({attributes}) \"/\" {name}\r\n").as_bytes())?;
    }
    session.push(format!("{tag} OK LSUB completed\r\n").as_bytes())
}

fn lsub_attributes(folder: &crate::store::SubscribedFolder) -> String {
    let special = imap_folders::special_use(folder.special_use);
    match (folder.exists, special.is_empty()) {
        (false, true) => "\\Noselect".to_owned(),
        (false, false) => format!("\\Noselect {special}"),
        (true, _) => special.to_owned(),
    }
}

fn wire_name(mailbox: imap::MailboxName) -> String {
    match mailbox {
        imap::MailboxName::Inbox => "INBOX".to_owned(),
        imap::MailboxName::Named(name) => name,
    }
}

#[allow(clippy::too_many_arguments)]
fn submit(
    session: &mut Session,
    request: Request,
    pending: Pending,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) {
    *request_id = request_id.wrapping_add(1).max(1);
    protocol::submit(session, request, pending, slot, generation, worker, owners);
}
