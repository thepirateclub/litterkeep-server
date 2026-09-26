//! Асинхронная SMTP AUTH PLAIN политика и передача проверки владельцу Store.
use super::*;

/// Проверяет ограничения AUTH и передаёт учётные данные storage worker-у.
pub(super) fn authenticate(
    session: &mut Session,
    response: Option<String>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    if session.tls.is_none() {
        return session.push(b"538 5.7.11 encryption required\r\n");
    }
    if session.authenticated_account.is_some() {
        return session.push(b"503 5.5.1 already authenticated\r\n");
    }
    let SmtpState::Command(transaction) = &session.smtp else {
        return session.push(b"503 5.5.1 bad sequence\r\n");
    };
    if transaction.has_sender() {
        return session.push(b"503 5.5.1 bad sequence\r\n");
    }
    let Some(response) = response else {
        session.smtp_auth = SmtpAuthState::AwaitingPlainResponse;
        return session.push(b"334 \r\n");
    };
    let credentials = match crate::sasl::parse_plain(&response) {
        Ok(credentials) => credentials,
        Err(_) => return session.push(b"501 5.5.2 invalid AUTH PLAIN response\r\n"),
    };
    let id = smtp_protocol::next_request(request_id);
    let account = match crate::local_route::local_login(&credentials.account) {
        Some(account) => account,
        None => return session.push(b"535 5.7.8 authentication credentials invalid\r\n"),
    };
    protocol::submit(
        session,
        Request::Authenticate {
            request_id: id,
            username: account.clone(),
            password: credentials.password,
        },
        Pending::SmtpAuthenticate { account },
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}
