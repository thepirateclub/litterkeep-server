//! Состояния SMTP wire-протокола и политика защищённой submission-сессии.
//!
//! Модуль не обращается к файловому хранилищу напрямую. Проверка учётных
//! данных и получателей всегда передаётся единственному владельцу store.
use super::session::DataSection;
use super::*;
use crate::local_route::{local_mailbox, local_recipient};

/// Обрабатывает одну завершённую CRLF строку SMTP либо строку DATA.
pub(super) fn smtp_line(
    session: &mut Session,
    line: Vec<u8>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    if matches!(session.smtp, SmtpState::Data { .. }) {
        return super::smtp_data::data_line(
            session, line, slot, generation, worker, owners, request_id,
        );
    }
    if matches!(session.smtp, SmtpState::BdatReady(_)) {
        return super::smtp_bdat::between_chunks(
            session, line, slot, generation, worker, owners, request_id,
        );
    }
    if matches!(session.smtp, SmtpState::BdatFailed) {
        return super::smtp_bdat::after_failure(session, &line);
    }
    if line.len() > smtp::command_line_limit(&line) {
        // Превышение лимита завершает текущий SASL exchange до ответа SMTP.
        if session.smtp_auth == SmtpAuthState::AwaitingPlainResponse {
            session.smtp_auth = SmtpAuthState::Command;
        }
        return session.push(b"500 5.5.2 command line too long\r\n");
    }
    let command = match smtp::parse(std::str::from_utf8(&line).unwrap_or("")) {
        Ok(command) => command,
        Err(smtp::ParseError::UnrecognizedCommand) => {
            return session.push(b"500 5.5.2 command unrecognized\r\n");
        }
        Err(smtp::ParseError::Syntax) => return session.push(b"501 5.5.2 syntax error\r\n"),
        Err(smtp::ParseError::SmtpUtf8Required) => {
            return session
                .push(b"550 5.6.7 internationalized address not permitted for sender\r\n");
        }
        Err(smtp::ParseError::UnsupportedParameter) => {
            return session.push(b"555 5.5.4 unsupported command parameter\r\n");
        }
        Err(smtp::ParseError::UnsupportedAuthenticationMechanism) => {
            return session.push(b"504 5.5.4 unsupported authentication mechanism\r\n");
        }
    };
    if let Some(reply) = super::smtp_greeting::requirement(session, &command) {
        return reply;
    }
    match command {
        smtp::Command::Quit => session.push(b"221 2.0.0 bye\r\n"),
        smtp::Command::Ehlo(_) => super::smtp_greeting::ehlo(session),
        smtp::Command::Helo(_) => super::smtp_greeting::helo(session),
        smtp::Command::Noop(_) => session.push(b"250 2.0.0 ok\r\n"),
        smtp::Command::Vrfy(_) => {
            session.push(b"252 2.5.2 cannot verify user, but will accept message\r\n")
        }
        smtp::Command::Expn(_) => session.push(b"502 5.5.1 mailing lists are not supported\r\n"),
        smtp::Command::Help(topic) => help(session, topic),
        smtp::Command::StartTls => super::smtp_greeting::starttls(session),
        smtp::Command::AuthPlain(response) => super::smtp_auth::authenticate(
            session, response, slot, generation, worker, owners, request_id,
        ),
        smtp::Command::Mail(sender) => mail(session, sender),
        smtp::Command::Bdat(chunk) => {
            super::smtp_bdat::start(session, chunk, slot, generation, worker, owners, request_id)
        }
        smtp::Command::Rcpt(address) => recipient(
            session, address, slot, generation, worker, owners, request_id,
        ),
        command => transaction_command(session, command),
    }
}

/// Возвращает справку RFC 5321 без влияния на активный mail transaction.
fn help(session: &mut Session, topic: Option<String>) -> io::Result<()> {
    match topic {
        Some(_) => session.push(b"214 2.0.0 use HELP for the command list\r\n"),
        None => session.push(
            b"214-2.0.0 Commands: EHLO HELO MAIL RCPT DATA BDAT RSET NOOP VRFY EXPN AUTH STARTTLS HELP QUIT\r\n214 2.0.0 End of HELP\r\n",
        ),
    }
}

/// Принимает единственную base64 response либо стандартную отмену SASL `*`.
pub(super) fn auth_plain_response(
    session: &mut Session,
    line: Vec<u8>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    session.smtp_auth = SmtpAuthState::Command;
    let response = std::str::from_utf8(&line)
        .ok()
        .and_then(|value| value.strip_suffix("\r\n"));
    let Some(response) = response else {
        return session.push(b"501 5.5.2 invalid AUTH PLAIN response\r\n");
    };
    match response {
        "*" => session.push(b"501 5.7.0 authentication cancelled\r\n"),
        _ => super::smtp_auth::authenticate(
            session,
            Some(response.to_owned()),
            slot,
            generation,
            worker,
            owners,
            request_id,
        ),
    }
}

/// Принимает MAIL только от аутентифицированного владельца локального адреса.
fn mail(session: &mut Session, mail: smtp::MailFrom) -> io::Result<()> {
    match mail_can_start_transaction(&session.smtp) {
        true => {}
        false => return session.push(b"503 5.5.1 mail transaction already open\r\n"),
    }
    let sender_is_authorized = match (
        session.tls.is_some(),
        session.authenticated_account.as_deref(),
    ) {
        (true, None) => return session.push(b"530 5.7.0 authentication required\r\n"),
        (false, _) => true,
        (true, Some(account)) => sender_is_authorized(&mail.reverse_path, account),
    };
    match sender_is_authorized {
        false => session.push(b"550 5.7.1 sender not authorized\r\n"),
        true => apply_mail(session, mail),
    }
}

/// Проверяет командный порядок до policy checks и не меняет открытый envelope.
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

/// Проверяет локального получателя через account owner перед изменением envelope.
fn recipient(
    session: &mut Session,
    address: String,
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
    let id = next_request(request_id);
    super::protocol::submit(
        session,
        Request::ValidateRecipient {
            request_id: id,
            mailbox,
        },
        Pending::ValidateRecipient { address },
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

/// Применяет команды DATA/RSET к уже проверенной транзакции.
fn transaction_command(session: &mut Session, command: smtp::Command) -> io::Result<()> {
    let data_for_binary_message = matches!(
        (&command, session.smtp_body_mode, &session.smtp),
        (
            smtp::Command::Data,
            smtp::BodyMode::BinaryMime,
            SmtpState::Command(transaction)
        ) if transaction.has_sender()
    );
    if data_for_binary_message {
        session.smtp = SmtpState::BdatFailed;
        return session.push(b"503 5.5.1 BINARYMIME requires BDAT; send RSET\r\n");
    }
    match (
        &command,
        session.tls.is_some(),
        session.authenticated_account.is_some(),
    ) {
        (smtp::Command::Data, true, false) => {
            return session.push(b"530 5.7.0 authentication required\r\n");
        }
        (smtp::Command::Data, _, _) | (smtp::Command::Rset, _, _) => {}
        _ => return session.push(b"503 5.5.1 bad sequence\r\n"),
    }
    apply_transaction(session, command)
}

fn apply_transaction(session: &mut Session, command: smtp::Command) -> io::Result<()> {
    let SmtpState::Command(transaction) = &mut session.smtp else {
        return session.push(b"503 5.5.1 bad sequence\r\n");
    };
    match transaction.apply(&command) {
        Ok(true) => {
            let envelope = transaction.take_envelope();
            session.smtp_data_activity = std::time::Instant::now();
            session.smtp = SmtpState::Data {
                bytes: Vec::new(),
                section: DataSection::Headers,
                recipients: envelope.recipients,
                reverse_path: envelope.reverse_path,
                oversized: false,
                body_mode: session.smtp_body_mode,
                smtp_utf8: session.smtp_utf8,
            };
            session.push(b"354 end with <CRLF>.<CRLF>\r\n")
        }
        Ok(false) => session.push(b"250 2.0.0 ok\r\n"),
        Err(_) => session.push(b"503 5.5.1 bad sequence\r\n"),
    }
}

/// Применяет принятую typed MAIL-команду через обычную transaction FSM.
fn apply_mail(session: &mut Session, mail: smtp::MailFrom) -> io::Result<()> {
    let body_mode = mail.body_mode;
    match mail.declared_size {
        Some(size) if size > smtp::MAX_MESSAGE_SIZE as u128 => {
            session.push(b"552 5.3.4 message size exceeds fixed maximum\r\n")
        }
        Some(_) | None => apply_mail_transaction(session, mail, body_mode),
    }
}

/// Переводит согласованный MAIL body mode в состояние ожидающей DATA.
fn apply_mail_transaction(
    session: &mut Session,
    mail: smtp::MailFrom,
    body_mode: smtp::BodyMode,
) -> io::Result<()> {
    let smtp_utf8 = mail.smtp_utf8;
    let reply = apply_transaction(session, smtp::Command::Mail(mail));
    match reply {
        Ok(()) => {
            match &session.smtp {
                SmtpState::Command(_) => session.smtp_body_mode = body_mode,
                SmtpState::Data { .. }
                | SmtpState::BdatDiscard { .. }
                | SmtpState::BdatReady(_)
                | SmtpState::BdatFailed
                | SmtpState::BdatReceiving(_) => {
                    return session.push(b"503 5.5.1 bad sequence\r\n");
                }
            }
            session.smtp_utf8 = smtp_utf8;
            Ok(())
        }
        Err(error) => Err(error),
    }
}

/// Выдаёт следующий ненулевой request id без дублирования арифметики.
pub(super) fn next_request(request_id: &mut u64) -> u64 {
    let current = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    current
}

#[cfg(test)]
#[path = "smtp_help_tests.rs"]
mod tests;
