//! Исполнение типизированных запросов единственным владельцем хранилища.
use super::{Request, Response};
use crate::{accounts::Accounts, store::Store};

/// Преобразует запрос reactor-а в completion, не выдавая Store наружу.
pub(super) fn respond(store: &mut Store, accounts: &Accounts, request: Request) -> Response {
    match request {
        Request::ScramVerifier(request) => {
            Response::ScramVerifier(super::sasl_lookup::load(accounts, request))
        }
        Request::ScramSha256Verifier(request) => {
            Response::ScramSha256Verifier(super::sasl_lookup::load_sha256(accounts, request))
        }
        Request::Authenticate {
            request_id,
            username,
            password,
        } => Response::Authenticated {
            request_id,
            result: accounts.authenticate(&username, &password),
        },
        Request::ValidateRecipient {
            request_id,
            mailbox,
        } => Response::RecipientValidated {
            request_id,
            accepted: crate::local_route::known_recipient(&mailbox, |name| accounts.exists(name)),
        },
        Request::Append {
            request_id,
            mailbox,
            bytes,
            metadata,
        } => Response::Appended {
            request_id,
            result: store.append_with_metadata(&mailbox, &bytes, metadata),
        },
        Request::ImapAppend {
            request_id,
            account,
            mailbox,
            messages,
        } => Response::ImapAppended {
            request_id,
            result: super::imap_append::execute(store, &account, &mailbox, &messages),
        },
        Request::AppendMany(request) => {
            super::local_delivery::append_many_response(store, accounts, *request)
        }
        Request::Read {
            request_id,
            mailbox,
        } => Response::Read {
            request_id,
            result: store.snapshot(&mailbox),
        },
        Request::UpdateFlags {
            request_id,
            mailbox,
            uids,
            mutation,
            flags,
            keywords,
        } => Response::FlagsUpdated {
            request_id,
            result: store
                .update_stored_flags(&mailbox, &uids, mutation, flags, &keywords)
                .and_then(|()| store.snapshot(&mailbox)),
        },
        Request::CreateFolder {
            request_id,
            account,
            name,
            special_use,
        } => Response::FolderCreated {
            request_id,
            result: match special_use {
                Some(value) => store.create_special_folder(&account, &name, value),
                None => store.create_folder(&account, &name),
            },
        },
        Request::RenameFolder {
            request_id,
            account,
            old_name,
            new_name,
        } => Response::FolderMutated {
            request_id,
            result: store.rename_folder(&account, &old_name, &new_name),
        },
        Request::DeleteFolder {
            request_id,
            account,
            name,
        } => Response::FolderMutated {
            request_id,
            result: store.delete_folder(&account, &name),
        },
        Request::ListFolders {
            request_id,
            account,
        } => Response::Folders {
            request_id,
            result: store.folders(&account),
        },
        Request::ResolveFolder {
            request_id,
            account,
            name,
        } => Response::FolderResolved {
            request_id,
            result: store.resolve_folder(&account, &name),
        },
        Request::ReadFolder {
            request_id,
            account,
            name,
        } => Response::FolderRead {
            request_id,
            result: store.snapshot_folder(&account, &name),
        },
        Request::SelectFolder {
            request_id,
            account,
            name,
        } => Response::FolderSelected {
            request_id,
            result: store.select_folder(&account, &name),
        },
        Request::SetSubscription {
            request_id,
            account,
            name,
            subscribed,
        } => Response::SubscriptionSet {
            request_id,
            result: store.set_subscription(&account, &name, subscribed),
        },
        Request::ListSubscriptions {
            request_id,
            account,
            include_status,
            include_size,
        } => Response::Subscriptions {
            request_id,
            result: store.subscription_snapshot_with_status(&account, include_status, include_size),
        },
        Request::Expunge {
            request_id,
            mailbox,
            uids,
        } => Response::Expunged {
            request_id,
            result: store.expunge(&mailbox, &uids),
        },
        Request::Copy {
            request_id,
            source,
            destination,
            uids,
        } => Response::Copied {
            request_id,
            result: store.copy(&source, &destination, &uids),
        },
        Request::Move {
            request_id,
            source,
            destination,
            uids,
        } => Response::Moved {
            request_id,
            result: store.move_messages(&source, &destination, &uids),
        },
        Request::SievePut {
            request_id,
            account,
            name,
            source,
        } => Response::SieveStored {
            request_id,
            result: store.put_sieve_script(&account, &name, source),
        },
        Request::SieveList {
            request_id,
            account,
        } => Response::SieveScripts {
            request_id,
            result: Ok(store.sieve_scripts(&account)),
        },
        Request::SieveSetActive {
            request_id,
            account,
            name,
        } => Response::SieveMutated {
            request_id,
            result: store.activate_sieve_script(&account, name.as_deref()),
        },
        Request::SieveGet {
            request_id,
            account,
            name,
        } => Response::SieveSource {
            request_id,
            result: store.sieve_script(&account, &name),
        },
        Request::SieveDelete {
            request_id,
            account,
            name,
        } => Response::SieveMutated {
            request_id,
            result: store.delete_sieve_script(&account, &name),
        },
        Request::SieveRename {
            request_id,
            account,
            old,
            new,
        } => Response::SieveMutated {
            request_id,
            result: store.rename_sieve_script(&account, &old, &new),
        },
        Request::SieveHaveSpace {
            request_id,
            account,
            name,
            size,
        } => Response::SieveSpace {
            request_id,
            result: store.sieve_have_space(&account, &name, size),
        },
        Request::SieveCheck { request_id, source } => Response::SieveMutated {
            request_id,
            result: store.check_sieve_script(&source),
        },
        Request::Dsn(super::message::DsnRequest::Failure {
            request_id,
            reverse_path,
            recipient,
            transaction_id,
            original,
            failure,
            dsn_metadata,
            diagnostic,
            report_key,
        }) => Response::Dsn(super::message::DsnResponse::Failure {
            request_id,
            result: super::dsn_delivery::append(
                store,
                accounts,
                crate::dsn::ReportRequest {
                    reverse_path: &reverse_path,
                    recipient: &recipient,
                    transaction_id,
                    original: &original,
                    failure,
                    metadata: dsn_metadata.as_deref(),
                    diagnostic: diagnostic.as_ref(),
                },
                &report_key,
            ),
        }),
        Request::Dsn(super::message::DsnRequest::Success { request_id, key }) => {
            Response::Dsn(super::message::DsnResponse::Success {
                request_id,
                result: super::dsn_delivery::append_success(store, accounts, key),
            })
        }
        Request::OutboundPending { request_id } => Response::OutboundPending {
            request_id,
            result: store.pending_outbound(),
        },
        Request::OutboundDue {
            request_id,
            now_epoch,
            limit,
        } => Response::OutboundDue {
            request_id,
            result: store.due_outbound_limited(now_epoch, limit),
        },
        Request::OutboundBlob {
            request_id,
            delivery,
        } => Response::OutboundBlob {
            request_id,
            result: store.outbound_blob(&delivery),
        },
        Request::OutboundTransition {
            request_id,
            delivery,
            state,
        } => Response::OutboundTransitioned {
            request_id,
            result: store.transition_outbound(&delivery, state),
        },
    }
}
