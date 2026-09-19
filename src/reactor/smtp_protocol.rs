//! Состояния SMTP wire-протокола и политика защищённой submission-сессии.
//!
//! Модуль не обращается к файловому хранилищу напрямую. Проверка учётных
//! данных и получателей всегда передаётся единственному владельцу store.
use super::local_delivery::{local_mailbox, local_mailboxes};
use super::*;

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
        Err(_) => return session.push(b"500 syntax error\r\n"),
    };
    match command {
        smtp::Command::Quit => session.push(b"221 bye\r\n"),
        smtp::Command::Ehlo(_) => ehlo(session),
        smtp::Command::Helo(_) => session.push(b"250 litterkeep.test\r\n"),
        smtp::Command::Noop => session.push(b"250 ok\r\n"),
        smtp::Command::AuthPlain(response) => authenticate(
            session, response, slot, generation, worker, owners, request_id,
        ),
        smtp::Command::Mail(address) => mail(session, address),
        smtp::Command::Rcpt(address) => recipient(
            session, address, slot, generation, worker, owners, request_id,
        ),
        command => transaction_command(session, command),
    }
}

/// Рекламирует AUTH только после implicit-TLS handshake.
fn ehlo(session: &mut Session) -> io::Result<()> {
    match session.tls {
        Some(_) => session.push(b"250-litterkeep.test\r\n250-AUTH PLAIN\r\n250 SIZE 10485760\r\n"),
        None => session.push(b"250-litterkeep.test\r\n250 SIZE 10485760\r\n"),
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
    match session.tls {
        None => return session.push(b"538 5.7.11 encryption required\r\n"),
        Some(_) => {}
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
    let account = credentials.account;
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
fn mail(session: &mut Session, address: String) -> io::Result<()> {
    match (
        session.tls.is_some(),
        session.authenticated_account.as_deref(),
    ) {
        (true, None) => return session.push(b"530 5.7.0 authentication required\r\n"),
        (false, _) => return apply_transaction(session, smtp::Command::Mail(address)),
        (true, Some(account)) => match local_mailbox(&address).as_deref() {
            Some(mailbox) if mailbox == account => {}
            Some(_) | None => return session.push(b"553 5.7.1 sender not authorized\r\n"),
        },
    }
    apply_transaction(session, smtp::Command::Mail(address))
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
    match (
        session.tls.is_some(),
        session.authenticated_account.is_none(),
    ) {
        (true, true) => return session.push(b"530 5.7.0 authentication required\r\n"),
        (true, false) | (false, _) => {}
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
    match (
        session.tls.is_some(),
        session.authenticated_account.is_none(),
    ) {
        (true, true) => return session.push(b"530 5.7.0 authentication required\r\n"),
        (true, false) | (false, _) => {}
    }
    apply_transaction(session, command)
}

fn apply_transaction(session: &mut Session, command: smtp::Command) -> io::Result<()> {
    let SmtpState::Command(transaction) = &mut session.smtp else {
        return session.push(b"503 bad sequence\r\n");
    };
    match transaction.apply(&command) {
        Ok(true) => {
            let recipients = transaction.take_recipients();
            session.smtp = SmtpState::Data {
                bytes: Vec::new(),
                recipients,
            };
            session.push(b"354 end with <CRLF>.<CRLF>\r\n")
        }
        Ok(false) => session.push(b"250 ok\r\n"),
        Err(_) => session.push(b"503 bad sequence\r\n"),
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
    let SmtpState::Data { bytes, recipients } = &mut session.smtp else {
        return Ok(());
    };
    match line.as_slice() {
        b".\r\n" => {
            let bytes = std::mem::take(bytes);
            let mailboxes = local_mailboxes(std::mem::take(recipients));
            session.smtp = SmtpState::Command(smtp::Transaction::default());
            let id = next_request(request_id);
            super::protocol::submit(
                session,
                Request::AppendMany {
                    request_id: id,
                    mailboxes,
                    bytes,
                },
                Pending::Append,
                slot,
                generation,
                worker,
                owners,
            );
        }
        _ if bytes.len().saturating_add(line.len()) > MAX_MESSAGE => {
            session.smtp = SmtpState::Command(smtp::Transaction::default());
            session.push(b"552 message too large\r\n")?;
        }
        _ => match line.strip_prefix(b"..") {
            Some(unstuffed) => bytes.extend_from_slice(unstuffed),
            None => bytes.extend_from_slice(&line),
        },
    }
    Ok(())
}

/// Выдаёт следующий ненулевой request id без дублирования арифметики.
fn next_request(request_id: &mut u64) -> u64 {
    let current = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    current
}
