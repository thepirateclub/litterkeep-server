//! Проверка client-first и submit verifier lookup для ManageSieve SCRAM.

use super::*;

/// Валидирует SASL client-first и передаёт только local account worker-у.
pub(super) fn start_lookup(
    session: &mut Session,
    response: String,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let message = match decode(response.as_bytes(), &response) {
        Some(message) => message,
        None => return invalid_first(session),
    };
    let client_first = match crate::sasl::scram_client_first::parse(&message) {
        Ok(client_first) => client_first,
        Err(_) => return invalid_first(session),
    };
    let account = match crate::local_route::local_login(&client_first.username) {
        Some(account) => account,
        None => return invalid_first(session),
    };
    match session
        .managesieve
        .begin_scram(account.clone(), client_first)
    {
        true => submit_lookup(
            session, account, slot, generation, worker, owners, request_id,
        ),
        false => invalid_first(session),
    }
}

/// Проверяет SHA-256 client-first и направляет lookup единственному owner.
pub(super) fn start_sha256_lookup(
    session: &mut Session,
    response: String,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let message = match decode(response.as_bytes(), &response) {
        Some(message) => message,
        None => return invalid_first(session),
    };
    let client_first = match crate::sasl::scram_client_first::parse(&message) {
        Ok(client_first) => client_first,
        Err(_) => return invalid_first(session),
    };
    let account = match crate::local_route::local_login(&client_first.username) {
        Some(account) => account,
        None => return invalid_first(session),
    };
    match session
        .managesieve
        .begin_scram_sha256(account.clone(), client_first)
    {
        true => submit_sha256_lookup(
            session, account, slot, generation, worker, owners, request_id,
        ),
        false => invalid_first(session),
    }
}

fn submit_sha256_lookup(
    session: &mut Session,
    account: String,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let id = managesieve_protocol::next(request_id);
    protocol::submit(
        session,
        Request::ScramSha256Verifier(crate::storage_worker::ScramSha256VerifierRequest {
            request_id: id,
            account,
        }),
        Pending::ManageSieveScramSha256Lookup,
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

/// Декодирует только canonical Base64 и UTF-8 SASL mechanism data.
fn decode(bytes: &[u8], encoded: &str) -> Option<String> {
    let decoded = crate::mime::decode_base64(bytes).ok()?;
    match crate::sasl::encode_base64(&decoded) == encoded {
        true => String::from_utf8(decoded).ok(),
        false => None,
    }
}

/// Ставит request во владельца Store и связывает его с connection generation.
fn submit_lookup(
    session: &mut Session,
    account: String,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let id = managesieve_protocol::next(request_id);
    protocol::submit(
        session,
        Request::ScramVerifier(crate::storage_worker::ScramVerifierRequest {
            request_id: id,
            account,
        }),
        Pending::ManageSieveScramLookup,
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

/// Сбрасывает некорректный exchange одинаковым отказом для malformed/unknown.
fn invalid_first(session: &mut Session) -> io::Result<()> {
    session.managesieve.scram_failed();
    session.push(b"NO \"authentication failed\"\r\n")
}
