//! Завершение SMTP pending-операций после проверки storage owner-ом.

use super::*;

pub(super) fn authenticated(
    session: &mut Session,
    account: String,
    accepted: bool,
) -> io::Result<()> {
    match accepted {
        true => {
            session.authenticated_account = Some(account);
            session.push(b"235 2.7.0 authentication successful\r\n")
        }
        false => session.push(b"535 5.7.8 authentication credentials invalid\r\n"),
    }
}

/// Завершает RCPT только после account policy, проверенной владельцем Store.
pub(super) fn accept_recipient(session: &mut Session, address: String) -> io::Result<()> {
    let SmtpState::Command(transaction) = &mut session.smtp else {
        return session.push(b"503 5.5.1 bad sequence\r\n");
    };
    match transaction.accept_recipient(address) {
        Ok(()) => session.push(b"250 2.1.5 recipient accepted\r\n"),
        Err(_) => session.push(b"503 5.5.1 bad sequence\r\n"),
    }
}
