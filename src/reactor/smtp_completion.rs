//! Завершение SMTP pending-операций после проверки storage owner-ом.

use super::*;

pub(super) fn authenticated(
    session: &mut Session,
    account: String,
    result: io::Result<bool>,
) -> io::Result<()> {
    match result {
        Ok(true) => {
            session.authenticated_account = Some(account);
            session.push(b"235 2.7.0 authentication successful\r\n")
        }
        Ok(false) => session.push(b"535 5.7.8 authentication credentials invalid\r\n"),
        Err(_) => session.push(b"454 4.7.0 authentication temporarily unavailable\r\n"),
    }
}

/// Завершает RCPT только после account policy, проверенной владельцем Store.
pub(super) fn accept_recipient(
    session: &mut Session,
    address: String,
    dsn: Option<crate::smtp::RcptTo>,
) -> io::Result<()> {
    let SmtpState::Command(transaction) = &mut session.smtp else {
        return session.push(b"503 5.5.1 bad sequence\r\n");
    };
    let accepted = match dsn {
        Some(recipient) => transaction.accept_typed_recipient(recipient),
        None => transaction.accept_recipient(address),
    };
    match accepted {
        Ok(()) => session.push(b"250 2.1.5 recipient accepted\r\n"),
        Err(_) => session.push(b"503 5.5.1 bad sequence\r\n"),
    }
}
