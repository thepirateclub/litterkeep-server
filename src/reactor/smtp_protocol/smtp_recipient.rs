//! Проверка MAIL/RCPT и сохранение recipient-scoped SMTP DSN параметров.

use super::super::*;
use crate::local_route::{local_mailbox, local_recipient};

/// Принимает MAIL только после sequence, authentication и sender policy checks.
pub(super) fn mail(
    session: &mut Session,
    mail: smtp::MailFrom,
    dsn: Option<smtp::MailDsn>,
) -> io::Result<()> {
    match mail_can_start_transaction(&session.smtp) {
        true => {}
        false => return session.push(b"503 5.5.1 mail transaction already open\r\n"),
    }
    let sender_authorized = match (
        session.tls.is_some(),
        session.authenticated_account.as_deref(),
    ) {
        (true, None) => return session.push(b"530 5.7.0 authentication required\r\n"),
        (false, _) => true,
        (true, Some(account)) => sender_is_authorized(&mail.reverse_path, account),
    };
    match sender_authorized {
        false => session.push(b"550 5.7.1 sender not authorized\r\n"),
        true => super::apply_mail(session, mail, dsn),
    }
}

/// Проверяет локального получателя до изменения SMTP transaction envelope.
#[allow(clippy::too_many_arguments)]
pub(super) fn recipient(
    session: &mut Session,
    address: String,
    dsn: Option<smtp::RcptTo>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    if let (true, true) = (
        session.tls.is_some(),
        session.authenticated_account.is_none(),
    ) {
        return session.push(b"530 5.7.0 authentication required\r\n");
    }
    let SmtpState::Command(transaction) = &session.smtp else {
        return session.push(b"503 5.5.1 bad sequence\r\n");
    };
    if !transaction.can_accept_recipient() {
        return session.push(b"503 5.5.1 bad sequence\r\n");
    }
    if !transaction.has_recipient_capacity() {
        return session.push(b"452 4.5.3 too many recipients\r\n");
    }
    if smtp::requires_smtp_utf8(&address) && !session.smtp_utf8 {
        return session.push(b"553 5.6.7 SMTPUTF8 required for recipient address\r\n");
    }
    let Some(mailbox) = local_recipient(&address) else {
        return session.push(b"550 5.1.1 unknown local recipient\r\n");
    };
    let id = super::next_request(request_id);
    super::super::protocol::submit(
        session,
        Request::ValidateRecipient {
            request_id: id,
            mailbox,
        },
        Pending::ValidateRecipient { address, dsn },
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

/// Проверяет командный порядок до mutation открытого SMTP envelope.
fn mail_can_start_transaction(state: &SmtpState) -> bool {
    match state {
        SmtpState::Command(transaction) => transaction.can_start_mail(),
        SmtpState::Data { .. }
        | SmtpState::BdatDiscard { .. }
        | SmtpState::BdatReady(_)
        | SmtpState::BdatFailed
        | SmtpState::BdatReceiving(_) => false,
    }
}

/// Проверяет envelope sender, сохраняя разрешённый RFC 6409 null reverse-path.
fn sender_is_authorized(reverse_path: &str, account: &str) -> bool {
    match reverse_path.is_empty() {
        true => true,
        false => {
            matches!(local_mailbox(reverse_path).as_deref(), Some(mailbox) if mailbox == account)
        }
    }
}
