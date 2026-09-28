use super::*;

pub(super) fn write_completion(
    session: &mut Session,
    pending: Pending,
    response: Response,
) -> io::Result<()> {
    let (pending, response) =
        match super::completion_managesieve::dispatch(session, pending, response) {
            super::completion_managesieve::Dispatch::Handled(result) => return result,
            super::completion_managesieve::Dispatch::Unhandled(pair) => *pair,
        };
    match (pending, response) {
        (
            Pending::Authenticate {
                tag,
                account,
                command,
            },
            Response::Authenticated {
                result: Ok(true), ..
            },
        ) => {
            session.authenticated_account = Some(account);
            let verb = match command {
                AuthenticationCommand::Login => "LOGIN",
                AuthenticationCommand::Authenticate => "AUTHENTICATE",
            };
            session.push(format!("{tag} OK {verb} completed\r\n").as_bytes())
        }
        (
            Pending::Authenticate { tag, .. },
            Response::Authenticated {
                result: Ok(false), ..
            },
        ) => session.push(format!("{tag} NO authentication failed\r\n").as_bytes()),
        (Pending::Authenticate { tag, .. }, Response::Authenticated { result: Err(_), .. }) => {
            session.push(format!("{tag} NO authentication temporarily unavailable\r\n").as_bytes())
        }
        (Pending::SmtpAuthenticate { account }, Response::Authenticated { result, .. }) => {
            smtp_completion::authenticated(session, account, result)
        }
        (
            Pending::ValidateRecipient { address, dsn },
            Response::RecipientValidated { accepted: true, .. },
        ) => smtp_completion::accept_recipient(session, address, dsn),
        (
            Pending::ValidateRecipient { .. },
            Response::RecipientValidated {
                accepted: false, ..
            },
        ) => session.push(b"550 5.1.1 unknown local recipient\r\n"),
        (Pending::Append { .. }, Response::Appended { result: Ok(_), .. }) => {
            session.push(b"250 2.0.0 queued\r\n")
        }
        (Pending::Append { .. }, Response::Appended { result: Err(_), .. }) => {
            session.push(b"451 4.3.0 storage failure\r\n")
        }
        (
            Pending::ImapAppend { tag, .. },
            Response::ImapAppended {
                result: Ok(result), ..
            },
        ) => {
            let uid_set = result
                .uids
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            session
                .push(format!("{tag} OK [APPENDUID 1 {uid_set}] APPEND completed\r\n").as_bytes())
        }
        (
            Pending::ImapAppend { tag, .. },
            Response::ImapAppended {
                result: Err(error), ..
            },
        ) => match error.kind() {
            io::ErrorKind::NotFound => {
                session.push(format!("{tag} NO [TRYCREATE] mailbox does not exist\r\n").as_bytes())
            }
            _ => session.push(format!("{tag} NO APPEND failed\r\n").as_bytes()),
        },
        (
            Pending::ReadForSelect { tag, read_only },
            Response::FolderSelected {
                result: Ok(resolved),
                ..
            },
        ) => {
            let snapshot = resolved.snapshot;
            let mailbox_flags = imap_select_flags::response(&snapshot.messages);
            let mailbox_name = imap_folders::quote(&resolved.name, session.imap_string_mode);
            let mailbox_attributes = imap_folders::special_use(resolved.special_use);
            session.selected_mailbox = Some(resolved.key);
            session.saved_search_uids.clear();
            session.selected_exists = snapshot.messages.len();
            session.selected_recent = resolved.recent_uids.into_iter().collect();
            session.selected_read_only = read_only;
            let (access, permanent_flags, permanent_flags_text) = match read_only {
                true => ("READ-ONLY", "", "No permanent flags permitted"),
                false => (
                    "READ-WRITE",
                    "\\Answered \\Flagged \\Deleted \\Seen \\Draft \\*",
                    "Flags permitted",
                ),
            };
            session.push(
                format!(
                    "* LIST ({mailbox_attributes}) \"/\" {mailbox_name}\r\n\
                     * FLAGS ({mailbox_flags})\r\n\
                     * {} EXISTS\r\n\
                     * {} RECENT\r\n\
                     * OK [UIDVALIDITY {}] UIDs valid\r\n\
                     * OK [UIDNEXT {}] Predicted next UID\r\n\
                     * OK [PERMANENTFLAGS ({permanent_flags})] {permanent_flags_text}\r\n\
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
            Pending::ReadForFetch {
                addressing,
                sequence_set,
                attributes,
                tag,
                saved_uids,
                ..
            },
            Response::Read {
                result: Ok(snapshot),
                ..
            },
        ) => imap_fetch_completion::write(
            session,
            imap_fetch_completion::Completion {
                addressing,
                sequence_set,
                attributes,
                tag,
                saved_uids: &saved_uids,
                messages: snapshot.messages,
                changed_uids: None,
            },
        ),
        (Pending::FetchReadForSeen { tag, .. }, Response::Read { .. }) => {
            session.push(format!("{tag} NO FETCH failed\r\n").as_bytes())
        }
        (pending @ Pending::FetchReadForSeen { .. }, response @ Response::FlagsUpdated { .. }) => {
            super::imap_fetch_seen::write(session, pending, response)
        }
        (
            Pending::ReadForSearch {
                addressing,
                key,
                charset,
                result,
                tag,
            },
            Response::Read {
                result: Ok(snapshot),
                ..
            },
        ) => imap_search::completed(
            session,
            addressing,
            key,
            charset,
            result,
            tag,
            snapshot.messages,
        ),
        (Pending::ReadForSearch { result, tag, .. }, Response::Read { result: Err(_), .. }) => {
            if result.saves_result() {
                session.saved_search_uids.clear();
            }
            session.push(format!("{tag} NO SEARCH failed\r\n").as_bytes())
        }
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
        (
            Pending::ListSubscriptions {
                tag,
                pattern,
                response,
            },
            Response::Subscriptions { result, .. },
        ) => imap_subscriptions::listed(session, tag, pattern, response, result),
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
                ..
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
                ..
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
