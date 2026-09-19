//! Исполнение типизированных запросов единственным владельцем хранилища.
use super::{Request, Response};
use crate::{accounts::Accounts, store::Store};
use std::io;
use std::sync::mpsc::{Receiver, Sender};

/// Последовательно исполняет запросы, пока владелец receiver-а не завершится.
pub(super) fn run(
    mut store: Store,
    accounts: Accounts,
    requests: Receiver<Request>,
    responses: Sender<Response>,
) {
    for request in requests {
        let response = respond(&mut store, &accounts, request);
        if responses.send(response).is_err() {
            break;
        }
    }
}

/// Преобразует запрос reactor-а в completion, не выдавая Store наружу.
pub(super) fn respond(store: &mut Store, accounts: &Accounts, request: Request) -> Response {
    match request {
        Request::Authenticate {
            request_id,
            username,
            password,
        } => Response::Authenticated {
            request_id,
            accepted: accounts.authenticate(&username, &password),
        },
        Request::ValidateRecipient {
            request_id,
            mailbox,
        } => Response::RecipientValidated {
            request_id,
            accepted: accounts.exists(&mailbox),
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
        Request::AppendMany {
            request_id,
            mailboxes,
            bytes,
        } => Response::Appended {
            request_id,
            result: append_many(store, accounts, mailboxes, &bytes),
        },
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
        } => Response::FlagsUpdated {
            request_id,
            result: store
                .update_flags(&mailbox, &uids, mutation, flags)
                .and_then(|()| store.snapshot(&mailbox)),
        },
    }
}

/// Сначала проверяет весь список получателей, затем публикует сообщение.
///
/// Предварительная проверка не допускает частичной доставки из-за опечатки в
/// последнем получателе. Ошибка ввода возвращается reactor-у без SMTP success.
fn append_many(
    store: &mut Store,
    accounts: &Accounts,
    mailboxes: Vec<String>,
    bytes: &[u8],
) -> io::Result<u64> {
    if mailboxes.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "no recipients"));
    }
    if !mailboxes.iter().all(|mailbox| accounts.exists(mailbox)) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "unknown local recipient",
        ));
    }
    store.append_many(&mailboxes, bytes)
}
