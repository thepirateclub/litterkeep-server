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
    result: io::Result<Vec<crate::store::SubscribedFolder>>,
) -> io::Result<()> {
    let subscriptions = match result {
        Ok(subscriptions) => subscriptions,
        Err(_) => {
            return match response {
                SubscriptionResponse::Lsub => {
                    session.push(format!("{tag} NO LSUB failed\r\n").as_bytes())
                }
                SubscriptionResponse::List => {
                    session.push(format!("{tag} NO LIST failed\r\n").as_bytes())
                }
            };
        }
    };
    for folder in subscriptions
        .iter()
        .filter(|folder| imap_list::matches_pattern(&folder.name, &pattern))
    {
        let attributes = attributes(folder, response);
        let name = imap_folders::quote(&folder.name);
        let line = match response {
            SubscriptionResponse::Lsub => format!("* LSUB ({attributes}) \"/\" {name}\r\n"),
            SubscriptionResponse::List => format!("* LIST ({attributes}) \"/\" {name}\r\n"),
        };
        session.push(line.as_bytes())?;
    }
    let completion = match response {
        SubscriptionResponse::Lsub => format!("{tag} OK LSUB completed\r\n"),
        SubscriptionResponse::List => format!("{tag} OK LIST completed\r\n"),
    };
    session.push(completion.as_bytes())
}

fn attributes(folder: &crate::store::SubscribedFolder, response: SubscriptionResponse) -> String {
    let special = imap_folders::special_use(folder.special_use);
    match (folder.exists, special.is_empty(), response) {
        (false, true, SubscriptionResponse::Lsub) => "\\Noselect".to_owned(),
        (false, false, SubscriptionResponse::Lsub) => format!("\\Noselect {special}"),
        (true, _, SubscriptionResponse::Lsub) => special.to_owned(),
        (false, true, SubscriptionResponse::List) => "\\NonExistent \\Subscribed".to_owned(),
        (false, false, SubscriptionResponse::List) => {
            format!("\\NonExistent \\Subscribed {special}")
        }
        (true, true, SubscriptionResponse::List) => "\\Subscribed".to_owned(),
        (true, false, SubscriptionResponse::List) => format!("\\Subscribed {special}"),
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
