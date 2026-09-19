//! Завершение SMTP pending-операций после проверки storage owner-ом.

use super::*;

/// Завершает RCPT только после account policy, проверенной владельцем Store.
pub(super) fn accept_recipient(session: &mut Session, address: String) -> io::Result<()> {
    let SmtpState::Command(transaction) = &mut session.smtp else {
        return session.push(b"503 bad sequence\r\n");
    };
    match transaction.accept_recipient(address) {
        Ok(()) => session.push(b"250 ok\r\n"),
        Err(_) => session.push(b"503 bad sequence\r\n"),
    }
}
