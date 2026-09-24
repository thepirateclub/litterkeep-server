//! Состояния SMTP wire-протокола и политика защищённой submission-сессии.
//!
//! Модуль не обращается к файловому хранилищу напрямую. Проверка учётных
//! данных и получателей всегда передаётся единственному владельцу store.
use super::*;
use crate::local_route::{local_mailbox, local_mailboxes};

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
        return data_line(session, line, slot, generation, worker, owners, request_id);
    }
    let command = match smtp::parse(std::str::from_utf8(&line).unwrap_or("")) {
        Ok(command) => command,
        Err(smtp::ParseError::Syntax) => return session.push(b"501 5.5.2 syntax error\r\n"),
        Err(smtp::ParseError::UnsupportedParameter) => {
            return session.push(b"555 5.5.4 unsupported command parameter\r\n");
        }
        Err(smtp::ParseError::UnsupportedAuthenticationMechanism) => {
            return session.push(b"504 5.5.4 unsupported authentication mechanism\r\n");
        }
    };
    match command {
        smtp::Command::Quit => session.push(b"221 bye\r\n"),
        smtp::Command::Ehlo(_) => ehlo(session),
        smtp::Command::Helo(_) => session.push(b"250 litterkeep.test\r\n"),
        smtp::Command::Noop => session.push(b"250 ok\r\n"),
        smtp::Command::StartTls => starttls(session),
        smtp::Command::AuthPlain(response) => authenticate(
            session, response, slot, generation, worker, owners, request_id,
        ),
        smtp::Command::Mail(sender) => mail(session, sender),
        smtp::Command::Rcpt(address) => recipient(
            session, address, slot, generation, worker, owners, request_id,
        ),
        command => transaction_command(session, command),
    }
}

/// Рекламирует AUTH только после implicit-TLS handshake.
fn ehlo(session: &mut Session) -> io::Result<()> {
    match (session.tls.is_some(), session.can_starttls()) {
        (true, _) => {
            session.push(b"250-litterkeep.test\r\n250-AUTH PLAIN\r\n250 SIZE 26214400\r\n")
        }
        (false, true) => {
            session.push(b"250-litterkeep.test\r\n250-STARTTLS\r\n250 SIZE 26214400\r\n")
        }
        (false, false) => session.push(b"250-litterkeep.test\r\n250 SIZE 26214400\r\n"),
    }
}

/// Переводит plaintext submission в TLS только вне активной транзакции.
fn starttls(session: &mut Session) -> io::Result<()> {
    let SmtpState::Command(transaction) = &session.smtp else {
        return session.push(b"503 5.5.1 bad sequence\r\n");
    };
    match (session.can_starttls(), transaction.has_sender()) {
        (true, false) => {
            session.push(b"220 2.0.0 ready to start TLS\r\n")?;
            session.request_starttls();
            Ok(())
        }
        (true, true) => session.push(b"503 5.5.1 bad sequence\r\n"),
        (false, _) => session.push(b"454 4.7.0 TLS not available\r\n"),
    }
}

/// Передаёт SASL PLAIN секрет worker-у без проверки в reactor thread.
fn authenticate(
    session: &mut Session,
    response: String,
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
    let credentials = match crate::sasl::parse_plain(&response) {
        Ok(credentials) => credentials,
        Err(_) => return session.push(b"501 5.5.2 invalid AUTH PLAIN response\r\n"),
    };
    let id = next_request(request_id);
    let account = match crate::local_route::local_login(&credentials.account) {
        Some(account) => account,
        None => return session.push(b"535 5.7.8 authentication credentials invalid\r\n"),
    };
    super::protocol::submit(
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

/// Принимает MAIL только от аутентифицированного владельца локального адреса.
fn mail(session: &mut Session, mail: smtp::MailFrom) -> io::Result<()> {
    let sender_is_authorized = match (
        session.tls.is_some(),
        session.authenticated_account.as_deref(),
    ) {
        (true, None) => return session.push(b"530 5.7.0 authentication required\r\n"),
        (false, _) => true,
        (true, Some(account)) => {
            matches!(local_mailbox(&mail.reverse_path).as_deref(), Some(mailbox) if mailbox == account)
        }
    };
    match sender_is_authorized {
        false => session.push(b"553 5.7.1 sender not authorized\r\n"),
        true => apply_mail(session, mail),
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
        return session.push(b"503 bad sequence\r\n");
    };
    if !transaction.can_accept_recipient() {
        return session.push(b"503 bad sequence\r\n");
    }
    let Some(mailbox) = local_mailbox(&address) else {
        return session.push(b"550 unknown local recipient\r\n");
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
    if let (true, true) = (
        session.tls.is_some(),
        session.authenticated_account.is_none(),
    ) {
        return session.push(b"530 5.7.0 authentication required\r\n");
    }
    apply_transaction(session, command)
}

fn apply_transaction(session: &mut Session, command: smtp::Command) -> io::Result<()> {
    let SmtpState::Command(transaction) = &mut session.smtp else {
        return session.push(b"503 bad sequence\r\n");
    };
    match transaction.apply(&command) {
        Ok(true) => {
            let envelope = transaction.take_envelope();
            session.smtp = SmtpState::Data {
                bytes: Vec::new(),
                recipients: envelope.recipients,
                reverse_path: envelope.reverse_path,
                oversized: false,
            };
            session.push(b"354 end with <CRLF>.<CRLF>\r\n")
        }
        Ok(false) => session.push(b"250 ok\r\n"),
        Err(_) => session.push(b"503 bad sequence\r\n"),
    }
}

/// Применяет принятую typed MAIL-команду через обычную transaction FSM.
fn apply_mail(session: &mut Session, mail: smtp::MailFrom) -> io::Result<()> {
    match mail.declared_size {
        Some(size) if size > smtp::MAX_MESSAGE_SIZE as u128 => {
            session.push(b"552 5.3.4 message size exceeds fixed maximum\r\n")
        }
        Some(_) | None => apply_transaction(session, smtp::Command::Mail(mail)),
    }
}

/// Накопление DATA ограничено независимо от скорости и размера TCP records.
fn data_line(
    session: &mut Session,
    line: Vec<u8>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let SmtpState::Data {
        bytes,
        recipients,
        reverse_path,
        oversized,
    } = &mut session.smtp
    else {
        return Ok(());
    };
    match line.as_slice() {
        b".\r\n" => {
            if *oversized {
                session.smtp = SmtpState::Command(smtp::Transaction::default());
                return session.push(b"552 5.3.4 message too large\r\n");
            }
            let bytes = std::mem::take(bytes);
            let reverse_path = std::mem::take(reverse_path);
            let mailboxes = local_mailboxes(std::mem::take(recipients));
            session.smtp = SmtpState::Command(smtp::Transaction::default());
            let bytes =
                crate::smtp_trace::prepend(&bytes, &reverse_path, std::time::SystemTime::now())?;
            let id = next_request(request_id);
            let pending = Pending::Append {
                mailboxes: mailboxes.clone(),
            };
            super::protocol::submit(
                session,
                Request::AppendMany {
                    request_id: id,
                    mailboxes,
                    envelope_sender: reverse_path,
                    bytes,
                },
                pending,
                slot,
                generation,
                worker,
                owners,
            );
        }
        _ if *oversized => {}
        _ => {
            let content = match line.strip_prefix(b"..") {
                Some(unstuffed) => unstuffed,
                None => line.as_slice(),
            };
            match bytes.len().saturating_add(content.len()) > MAX_MESSAGE {
                true => *oversized = true,
                false => bytes.extend_from_slice(content),
            }
        }
    }
    Ok(())
}

/// Выдаёт следующий ненулевой request id без дублирования арифметики.
fn next_request(request_id: &mut u64) -> u64 {
    let current = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    current
}
