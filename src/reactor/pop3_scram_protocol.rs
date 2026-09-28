//! Асинхронные POP3 SCRAM lookup и переход к владельцу maildrop.

use super::*;

/// Ставит verifier lookup в единственный StorageWorker владельца credential.
#[allow(clippy::too_many_arguments)]
pub(super) fn lookup(
    session: &mut Session,
    account: String,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let id = pop3_protocol::next_id(request_id);
    protocol::submit(
        session,
        Request::ScramVerifier(crate::storage_worker::ScramVerifierRequest {
            request_id: id,
            account,
        }),
        Pending::Pop3ScramLookup,
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

/// Запрашивает SHA-256 verifier через тот же единственный Accounts owner.
#[allow(clippy::too_many_arguments)]
pub(super) fn lookup_sha256(
    session: &mut Session,
    account: String,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let id = pop3_protocol::next_id(request_id);
    protocol::submit(
        session,
        Request::ScramSha256Verifier(crate::storage_worker::ScramSha256VerifierRequest {
            request_id: id,
            account,
        }),
        Pending::Pop3ScramSha256Lookup,
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

/// Получает POP3 maildrop lease только после mutual SCRAM proof.
#[allow(clippy::too_many_arguments)]
pub(super) fn authenticated(
    session: &mut Session,
    account: String,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    read_inbox(
        session, account, slot, generation, worker, owners, request_id,
    )
}

/// Загружает snapshot после proof; completion захватит lease до публикации клиенту.
#[allow(clippy::too_many_arguments)]
fn read_inbox(
    session: &mut Session,
    account: String,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let id = pop3_protocol::next_id(request_id);
    let pending_account = account.clone();
    protocol::submit(
        session,
        Request::ReadFolder {
            request_id: id,
            account,
            name: "INBOX".into(),
        },
        Pending::Pop3ScramReadInbox {
            account: pending_account,
        },
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}
