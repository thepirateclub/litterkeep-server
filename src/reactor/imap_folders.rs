//! Асинхронная маршрутизация CREATE/LIST через единственного владельца store.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn create(
    session: &mut Session,
    tag: String,
    mailbox: imap::MailboxName,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let Some(account) = session.authenticated_account.clone() else {
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    let name = match mailbox {
        imap::MailboxName::Inbox => {
            return session.push(format!("{tag} NO [ALREADYEXISTS] INBOX exists\r\n").as_bytes());
        }
        imap::MailboxName::Named(name) => name,
    };
    submit_folder(
        session,
        Request::CreateFolder {
            request_id: *request_id,
            account,
            name,
        },
        Pending::CreateFolder { tag },
        slot,
        generation,
        worker,
        owners,
        request_id,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn delete(
    session: &mut Session,
    tag: String,
    mailbox: imap::MailboxName,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let Some(account) = session.authenticated_account.clone() else {
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    let name = match mailbox {
        imap::MailboxName::Inbox => {
            return session
                .push(format!("{tag} NO [CANNOT] INBOX cannot be deleted\r\n").as_bytes());
        }
        imap::MailboxName::Named(name) => name,
    };
    submit_folder(
        session,
        Request::DeleteFolder {
            request_id: *request_id,
            account,
            name,
        },
        Pending::MutateFolder {
            tag,
            operation: imap::folder::Mutation::Delete,
        },
        slot,
        generation,
        worker,
        owners,
        request_id,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn rename(
    session: &mut Session,
    tag: String,
    source: imap::MailboxName,
    destination: imap::MailboxName,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let Some(account) = session.authenticated_account.clone() else {
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    let (old_name, new_name) = match (source, destination) {
        (imap::MailboxName::Inbox, _) => {
            return session
                .push(format!("{tag} NO [CANNOT] INBOX cannot be renamed\r\n").as_bytes());
        }
        (_, imap::MailboxName::Inbox) => {
            return session.push(format!("{tag} NO [ALREADYEXISTS] INBOX exists\r\n").as_bytes());
        }
        (imap::MailboxName::Named(old), imap::MailboxName::Named(new)) => (old, new),
    };
    submit_folder(
        session,
        Request::RenameFolder {
            request_id: *request_id,
            account,
            old_name,
            new_name,
        },
        Pending::MutateFolder {
            tag,
            operation: imap::folder::Mutation::Rename,
        },
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
) -> io::Result<()> {
    let Some(account) = session.authenticated_account.clone() else {
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    submit_folder(
        session,
        Request::ListFolders {
            request_id: *request_id,
            account,
        },
        Pending::ListFolders { tag, pattern },
        slot,
        generation,
        worker,
        owners,
        request_id,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn submit_folder(
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

pub(super) fn created(
    session: &mut Session,
    tag: String,
    result: io::Result<crate::store::FolderInfo>,
) -> io::Result<()> {
    match result {
        Ok(_) => session.push(format!("{tag} OK CREATE completed\r\n").as_bytes()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            session.push(format!("{tag} NO [ALREADYEXISTS] folder exists\r\n").as_bytes())
        }
        Err(_) => session.push(format!("{tag} NO CREATE failed\r\n").as_bytes()),
    }
}

pub(super) fn mutated(
    session: &mut Session,
    tag: String,
    operation: imap::folder::Mutation,
    result: io::Result<()>,
) -> io::Result<()> {
    let verb = match operation {
        imap::folder::Mutation::Delete => "DELETE",
        imap::folder::Mutation::Rename => "RENAME",
    };
    match result {
        Ok(()) => session.push(format!("{tag} OK {verb} completed\r\n").as_bytes()),
        Err(error) => {
            let code = match error.kind() {
                io::ErrorKind::NotFound => "NONEXISTENT",
                io::ErrorKind::AlreadyExists => "ALREADYEXISTS",
                io::ErrorKind::InvalidInput | io::ErrorKind::PermissionDenied => "CANNOT",
                _ => "SERVERBUG",
            };
            session.push(format!("{tag} NO [{code}] {verb} failed\r\n").as_bytes())
        }
    }
}

pub(super) fn listed(
    session: &mut Session,
    tag: String,
    pattern: imap::folder::ListPattern,
    result: io::Result<Vec<crate::store::FolderInfo>>,
) -> io::Result<()> {
    let folders = match result {
        Ok(folders) => folders,
        Err(_) => return session.push(format!("{tag} NO LIST failed\r\n").as_bytes()),
    };
    for folder in folders
        .iter()
        .filter(|folder| matches_pattern(&folder.name, &pattern))
    {
        let attributes = special_use(folder.special_use);
        let name = quote(&folder.name);
        session.push(format!("* LIST ({attributes}) \"/\" {name}\r\n").as_bytes())?;
    }
    session.push(format!("{tag} OK LIST completed\r\n").as_bytes())
}

pub(super) fn matches_pattern(name: &str, pattern: &imap::folder::ListPattern) -> bool {
    match pattern {
        imap::folder::ListPattern::All => true,
        imap::folder::ListPattern::OneLevel => !name.contains('/'),
        imap::folder::ListPattern::Exact(expected) => name == expected,
    }
}

pub(super) fn special_use(value: Option<crate::store::SpecialUse>) -> &'static str {
    match value {
        Some(crate::store::SpecialUse::Inbox) | None => "",
        Some(crate::store::SpecialUse::Sent) => "\\Sent",
        Some(crate::store::SpecialUse::Drafts) => "\\Drafts",
        Some(crate::store::SpecialUse::Trash) => "\\Trash",
        Some(crate::store::SpecialUse::Junk) => "\\Junk",
        Some(crate::store::SpecialUse::Archive) => "\\Archive",
    }
}

pub(super) fn quote(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}
