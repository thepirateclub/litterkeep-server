//! Завершает проверку SMTP DATA/RSET и ограничивает срок жизни MAIL-маркеров.
use super::*;

/// Применяет команды DATA/RSET после общего разбора и проверки порядка команд.
///
/// Маркеры `BODY` и `SMTPUTF8` живут только в текущем MAIL-конверте. Перед
/// DATA они копируются в состояние сообщения, а после RSET сбрасываются,
/// чтобы следующая транзакция не унаследовала расширения прежнего отправителя.
pub(super) fn transaction_command(session: &mut Session, command: smtp::Command) -> io::Result<()> {
    let resets_transaction = matches!(&command, smtp::Command::Rset);
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
    let result = super::smtp_protocol::apply_transaction(session, command);
    if resets_transaction {
        session.smtp_body_mode = smtp::BodyMode::SevenBit;
        session.smtp_utf8 = false;
    }
    result
}
