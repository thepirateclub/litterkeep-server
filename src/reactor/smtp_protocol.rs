//! Состояния SMTP wire-протокола и политика защищённой submission-сессии.
//!
//! Модуль не обращается к файловому хранилищу напрямую. Проверка учётных
//! данных и получателей всегда передаётся единственному владельцу store.
use super::session::DataSection;
use super::*;
#[path = "smtp_protocol/smtp_recipient.rs"]
mod smtp_recipient;

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
        smtp::Command::Mail(sender) => smtp_recipient::mail(session, sender, None),
        smtp::Command::MailWithDsn(mail, dsn) => smtp_recipient::mail(session, mail, Some(dsn)),
        smtp::Command::Bdat(chunk) => {
            super::smtp_bdat::start(session, chunk, slot, generation, worker, owners, request_id)
        }
        smtp::Command::Rcpt(address) => smtp_recipient::recipient(
            session, address, None, slot, generation, worker, owners, request_id,
        ),
        smtp::Command::RcptWithDsn(recipient) => smtp_recipient::recipient(
            session,
            recipient.address.clone(),
            Some(recipient),
            slot,
            generation,
            worker,
            owners,
            request_id,
        ),
        command => super::smtp_transaction_command::transaction_command(session, command),
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

/// Применяет проверенную MAIL команду к transaction FSM.
pub(super) fn apply_transaction(session: &mut Session, command: smtp::Command) -> io::Result<()> {
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
                dsn: envelope.dsn,
                reverse_path: envelope.reverse_path,
                oversized: false,
                invalid_octets: false,
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
pub(super) fn apply_mail(
    session: &mut Session,
    mail: smtp::MailFrom,
    dsn: Option<smtp::MailDsn>,
) -> io::Result<()> {
    let body_mode = mail.body_mode;
    match mail.declared_size {
        Some(size) if size > smtp::MAX_MESSAGE_SIZE as u128 => {
            session.push(b"552 5.3.4 message size exceeds fixed maximum\r\n")
        }
        Some(_) | None => apply_mail_transaction(session, mail, body_mode, dsn),
    }
}

/// Переводит согласованный MAIL body mode в состояние ожидающей DATA.
fn apply_mail_transaction(
    session: &mut Session,
    mail: smtp::MailFrom,
    body_mode: smtp::BodyMode,
    dsn: Option<smtp::MailDsn>,
) -> io::Result<()> {
    let smtp_utf8 = mail.smtp_utf8;
    let reply = apply_transaction(session, smtp::Command::Mail(mail));
    match reply {
        Ok(()) => {
            match &mut session.smtp {
                SmtpState::Command(transaction) if transaction.has_sender() => {
                    if let Some(parameters) = dsn {
                        transaction.set_mail_dsn(parameters);
                    }
                    session.smtp_body_mode = body_mode;
                }
                SmtpState::Command(_) => {}
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
