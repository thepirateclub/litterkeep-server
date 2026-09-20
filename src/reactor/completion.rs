use super::*;

pub(super) fn write_completion(
    session: &mut Session,
    pending: Pending,
    response: Response,
) -> io::Result<()> {
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
        (Pending::SmtpAuthenticate { account }, Response::Authenticated { accepted, .. }) => {
            smtp_completion::authenticated(session, account, accepted)
        }
        (
            Pending::ManageSieveAuthenticate { account },
            Response::Authenticated { accepted, .. },
        ) => managesieve_completion::authenticate(session, account, accepted),
        (Pending::ManageSieveCommand, Response::SieveStored { result, .. }) => {
            managesieve_completion::result(session, result.map(|_| ()))
        }
        (Pending::ManageSieveCommand, Response::SieveMutated { result, .. }) => {
            managesieve_completion::result(session, result)
        }
        (Pending::ManageSieveCommand, Response::SieveSpace { result, .. }) => match result {
            Ok(true) => session.push(b"OK \"space available\"\r\n"),
            Ok(false) => session.push(b"NO (QUOTA) \"space unavailable\"\r\n"),
            Err(_) => session.push(b"NO \"storage failure\"\r\n"),
        },
        (Pending::ManageSieveCommand, Response::SieveScripts { result, .. }) => {
            managesieve_completion::list(session, result)
        }
        (Pending::ManageSieveCommand, Response::SieveSource { result, .. }) => {
            managesieve_completion::source(session, result)
        }
        (
            Pending::ValidateRecipient { address },
            Response::RecipientValidated { accepted: true, .. },
        ) => smtp_completion::accept_recipient(session, address),
        (
            Pending::ValidateRecipient { .. },
            Response::RecipientValidated {
                accepted: false, ..
            },
        ) => session.push(b"550 unknown local recipient\r\n"),
        (Pending::Append { .. }, Response::Appended { result: Ok(_), .. }) => {
            session.push(b"250 queued\r\n")
        }
        (Pending::Append { .. }, Response::Appended { result: Err(_), .. }) => {
            session.push(b"451 storage failure\r\n")
        }
        (
            Pending::ImapAppend { tag, .. },
            Response::Appended {
                result: Ok(uid), ..
            },
        ) => session.push(format!("{tag} OK [APPENDUID 1 {uid}] APPEND completed\r\n").as_bytes()),
        (Pending::ImapAppend { tag, .. }, Response::Appended { result: Err(_), .. }) => {
            session.push(format!("{tag} NO APPEND failed\r\n").as_bytes())
        }
        (
            Pending::ReadForSelect { tag, read_only },
            Response::FolderSelected {
                result: Ok(resolved),
                ..
            },
        ) => {
            let snapshot = resolved.snapshot;
            session.selected_mailbox = Some(resolved.key);
            session.selected_exists = snapshot.messages.len();
            session.selected_recent = resolved.recent_uids.into_iter().collect();
            session.selected_read_only = read_only;
            let access = match read_only {
                true => "READ-ONLY",
                false => "READ-WRITE",
            };
            session.push(
                format!(
                    "* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n\
                     * {} EXISTS\r\n\
                     * {} RECENT\r\n\
                     * OK [UIDVALIDITY {}] UIDs valid\r\n\
                     * OK [UIDNEXT {}] Predicted next UID\r\n\
                     * OK [PERMANENTFLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)] Flags permitted\r\n\
                     {tag} OK [{access}] SELECT completed\r\n",
                    snapshot.messages.len(),
                    session.selected_recent.len(),
                    snapshot.uid_validity,
                    snapshot.uid_next,
                )
                .as_bytes(),
            )
        }
        (
            Pending::ReadForSelect { tag, .. },
            Response::FolderSelected {
                result: Err(error), ..
            },
        ) => match error.kind() {
            io::ErrorKind::NotFound => {
                session.push(format!("{tag} NO mailbox does not exist\r\n").as_bytes())
            }
            _ => session.push(format!("{tag} NO SELECT failed\r\n").as_bytes()),
        },
        (
            Pending::ResolveForAppend {
                tag,
                literal_size,
                metadata,
            },
            Response::FolderResolved { result, .. },
        ) => imap_append::resolved(session, tag, literal_size, metadata, result),
        (
            Pending::ReadForFetch(addressing, sequence_set, attribute, tag),
            Response::Read {
                result: Ok(snapshot),
                ..
            },
        ) => imap_fetch_completion::write(
            session,
            addressing,
            sequence_set,
            attribute,
            tag,
            snapshot.messages,
        ),
        (
            Pending::ReadForSearch {
                addressing,
                key,
                tag,
            },
            Response::Read {
                result: Ok(snapshot),
                ..
            },
        ) => imap_search::completed(session, addressing, key, tag, snapshot.messages),
        (_, Response::Read { .. }) => session.push(b"451 storage failure\r\n"),
        (pending @ Pending::WriteFlags { .. }, response @ Response::FlagsUpdated { .. }) => {
            super::flag_completion::write(session, pending, response)
        }
        (_, Response::FlagsUpdated { .. }) => session.push(b"451 storage failure\r\n"),
        (Pending::CreateFolder { tag }, Response::FolderCreated { result, .. }) => {
            imap_folders::created(session, tag, result)
        }
        (Pending::MutateFolder { tag, operation }, Response::FolderMutated { result, .. }) => {
            imap_folders::mutated(session, tag, operation, result)
        }
        (
            Pending::ListFolders {
                tag,
                pattern,
                special_use_only,
            },
            Response::Folders { result, .. },
        ) => imap_list::listed(session, tag, pattern, special_use_only, result),
        (
            Pending::SetSubscription { tag, subscribed },
            Response::SubscriptionSet { result, .. },
        ) => imap_subscriptions::set_completed(session, tag, subscribed, result),
        (Pending::ListSubscriptions { tag, pattern }, Response::Subscriptions { result, .. }) => {
            imap_subscriptions::listed(session, tag, pattern, result)
        }
        (
            Pending::Status {
                tag,
                name,
                attributes,
            },
            Response::FolderRead { result, .. },
        ) => imap_status::completed(session, tag, name, attributes, result),
        (
            Pending::Expunge {
                tag,
                mailbox: _,
                sequence_numbers,
                close,
            },
            Response::Expunged { result, .. },
        ) => imap_expunge::completed(session, tag, sequence_numbers, result, close),
        (
            Pending::Copy {
                tag,
                transfer,
                source_uids,
                source_sequences,
            },
            Response::Copied { result, .. },
        ) => imap_copy::completed(
            session,
            tag,
            transfer,
            source_uids,
            source_sequences,
            result,
        ),
        (
            Pending::Copy {
                tag,
                transfer,
                source_uids,
                source_sequences,
            },
            Response::Moved { result, .. },
        ) => imap_copy::completed(
            session,
            tag,
            transfer,
            source_uids,
            source_sequences,
            result,
        ),
        (
            Pending::ResolveCopyDestination { tag, .. },
            Response::FolderResolved { result: Err(_), .. },
        ) => imap_copy::destination_missing(session, tag),
        _ => Ok(()),
    }
}
