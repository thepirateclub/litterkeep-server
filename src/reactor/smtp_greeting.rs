//! SMTP greeting negotiation, ESMTP gating and STARTTLS transition.
use super::*;

/// Применяет обязательный greeting gate к transaction и ESMTP командам.
pub(super) fn requirement(
    session: &mut Session,
    command: &smtp::Command,
) -> Option<io::Result<()>> {
    match (session.smtp_greeting, command) {
        (
            smtp::GreetingMode::NotEstablished,
            smtp::Command::Mail(_)
            | smtp::Command::Rcpt(_)
            | smtp::Command::Data
            | smtp::Command::Bdat(_)
            | smtp::Command::AuthPlain(_)
            | smtp::Command::StartTls,
        ) => Some(session.push(b"503 5.5.1 send HELO or EHLO first\r\n")),
        (
            smtp::GreetingMode::Basic,
            smtp::Command::AuthPlain(_) | smtp::Command::StartTls | smtp::Command::Bdat(_),
        ) => Some(session.push(b"503 5.5.1 EHLO required for this extension\r\n")),
        (smtp::GreetingMode::Basic, smtp::Command::Mail(mail)) => {
            match (mail.declared_size.is_some(), mail.body_mode, mail.smtp_utf8) {
                (true, _, _) | (_, smtp::BodyMode::EightBitMime, _) | (_, _, true) => {
                    Some(session.push(b"555 5.5.4 ESMTP parameter requires EHLO\r\n"))
                }
                (false, smtp::BodyMode::SevenBit, false) => None,
            }
        }
        _ => None,
    }
}

/// Сбрасывает envelope и принимает обычный RFC 5321 HELO greeting.
pub(super) fn helo(session: &mut Session) -> io::Result<()> {
    establish(
        session,
        smtp::GreetingMode::Basic,
        b"250 litterkeep.test\r\n",
    )
}

/// Рекламирует только поддержанные расширения и очищает предыдущий envelope.
pub(super) fn ehlo(session: &mut Session) -> io::Result<()> {
    let response: &[u8] = match (session.tls.is_some(), session.can_starttls()) {
        (true, _) => {
            b"250-litterkeep.test\r\n250-PIPELINING\r\n250-CHUNKING\r\n250-ENHANCEDSTATUSCODES\r\n250-AUTH PLAIN\r\n250-8BITMIME\r\n250 SIZE 26214400\r\n"
        }
        (false, true) => {
            b"250-litterkeep.test\r\n250-PIPELINING\r\n250-STARTTLS\r\n250-CHUNKING\r\n250-ENHANCEDSTATUSCODES\r\n250-8BITMIME\r\n250 SIZE 26214400\r\n"
        }
        (false, false) => {
            b"250-litterkeep.test\r\n250-PIPELINING\r\n250-CHUNKING\r\n250-ENHANCEDSTATUSCODES\r\n250-8BITMIME\r\n250 SIZE 26214400\r\n"
        }
    };
    establish(session, smtp::GreetingMode::Extended, response)
}

/// Переводит plaintext submission в TLS только после EHLO и вне envelope.
pub(super) fn starttls(session: &mut Session) -> io::Result<()> {
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

/// Завершает greeting: его успешное принятие сбрасывает прошлую транзакцию.
fn establish(session: &mut Session, mode: smtp::GreetingMode, response: &[u8]) -> io::Result<()> {
    session.push(response)?;
    session.smtp = SmtpState::Command(smtp::Transaction::default());
    session.smtp_greeting = mode;
    session.smtp_body_mode = smtp::BodyMode::SevenBit;
    session.smtp_utf8 = false;
    Ok(())
}
