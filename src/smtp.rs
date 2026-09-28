//! SMTP submission transaction state.
#[path = "smtp/command.rs"]
mod command;
#[path = "smtp/dsn_envelope.rs"]
mod dsn_envelope;
#[path = "smtp/idna17.rs"]
pub mod idna17;
#[path = "smtp/punycode.rs"]
pub mod punycode;
pub(crate) use command::canonical_parts;
pub(crate) use command::is_utf8_mailbox;
pub(crate) use command::requires_smtp_utf8;
pub use command::{
    BdatCommand, BodyMode, Command, MAX_BINARYMIME_MAIL_LINE_SIZE, MAX_COMMAND_LINE_SIZE,
    MAX_DATA_LINE_SIZE, MAX_LOCAL_PART_SIZE, MAX_MESSAGE_SIZE, MAX_PATH_SIZE,
    MAX_SIZE_BINARYMIME_MAIL_LINE_SIZE, MAX_SIZE_MAIL_LINE_SIZE,
    MAX_SIZE_SMTPUTF8_BINARYMIME_MAIL_LINE_SIZE, MAX_SIZE_SMTPUTF8_MAIL_LINE_SIZE,
    MAX_SMTPUTF8_BINARYMIME_MAIL_LINE_SIZE, MAX_SMTPUTF8_MAIL_LINE_SIZE, MailDsn, MailFrom,
    NotifyCondition, NotifyPolicy, OriginalAddressType, OriginalRecipient, ParseError, RcptTo,
    RecipientDsn, ReplyEncoding, ReturnContent, VerifyRequest, command_line_limit, parse,
};

/// Уровень SMTP greeting, определяющий допустимость ESMTP extensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GreetingMode {
    NotEstablished,
    Basic,
    Extended,
}

/// Проверенный envelope, передаваемый из SMTP FSM в локальную delivery.
#[derive(Debug, Eq, PartialEq)]
pub struct Envelope {
    pub reverse_path: String,
    pub recipients: Vec<String>,
    pub dsn: DsnEnvelope,
}

/// Typed SMTP envelope DSN, привязываемый Store к transaction ID доставки.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DsnEnvelope {
    /// Обратный SMTP путь, а не MIME поле `From`.
    pub reverse_path: String,
    /// Необязательные параметры RFC 3461 команды MAIL.
    pub mail: MailDsn,
    /// Получатели вместе с независимыми запросами NOTIFY и ORCPT.
    pub recipients: Vec<RcptTo>,
}
/// Верхняя граница получателей одного SMTP envelope в локальном профиле.
const MAX_RECIPIENTS: usize = 100;
#[derive(Default, Debug)]
pub struct Transaction {
    sender: Option<String>,
    recipients: Vec<String>,
    mail_dsn: MailDsn,
    recipient_dsn: Vec<RcptTo>,
}
impl Transaction {
    pub fn apply(&mut self, command: &Command) -> Result<bool, &'static str> {
        match command {
            Command::Mail(_) | Command::MailWithDsn(_, _) if self.sender.is_some() => {
                Err("mail transaction already open")
            }
            Command::Mail(mail) => self.accept_mail(mail, MailDsn::default()),
            Command::MailWithDsn(mail, dsn) => self.accept_mail(mail, dsn.clone()),
            Command::Rcpt(r) if self.sender.is_some() => {
                self.accept_recipient(r.clone())?;
                Ok(false)
            }
            Command::RcptWithDsn(recipient) if self.sender.is_some() => {
                self.accept_typed_recipient(recipient.clone())?;
                Ok(false)
            }
            Command::Data if !self.recipients.is_empty() => Ok(true),
            Command::Rset => {
                *self = Self::default();
                Ok(false)
            }
            _ => Err("bad command sequence"),
        }
    }

    /// Забирает получателей принятой команды DATA и закрывает транзакцию.
    ///
    /// После этого SMTP-сессия уже не может повторно использовать прежний
    /// envelope для следующего сообщения.
    pub fn take_envelope(&mut self) -> Envelope {
        let reverse_path = self.sender.take().unwrap_or_default();
        let recipients = std::mem::take(&mut self.recipients);
        Envelope {
            reverse_path: reverse_path.clone(),
            recipients,
            dsn: DsnEnvelope {
                reverse_path,
                mail: std::mem::take(&mut self.mail_dsn),
                recipients: std::mem::take(&mut self.recipient_dsn),
            },
        }
    }

    /// Проверяет, можно ли принять очередного RCPT в текущем envelope.
    pub fn can_accept_recipient(&self) -> bool {
        self.sender.is_some()
    }

    /// Проверяет наличие места до асинхронной проверки account policy.
    pub fn has_recipient_capacity(&self) -> bool {
        self.recipients.len() < MAX_RECIPIENTS
    }

    /// Сообщает, начата ли envelope-транзакция командой MAIL.
    pub fn has_sender(&self) -> bool {
        self.sender.is_some()
    }

    /// Разрешает DATA/BDAT только после MAIL и хотя бы одного принятого RCPT.
    pub fn can_start_data(&self) -> bool {
        matches!(
            (self.sender.is_some(), self.recipients.is_empty()),
            (true, false)
        )
    }

    /// Разрешает MAIL только после завершения или сброса прежнего конверта.
    pub fn can_start_mail(&self) -> bool {
        self.sender.is_none()
    }

    /// Добавляет проверенный policy слоем получатель в активный envelope.
    pub fn accept_recipient(&mut self, recipient: String) -> Result<(), &'static str> {
        if !self.can_accept_recipient() {
            return Err("bad command sequence");
        }
        if self.recipients.len() >= MAX_RECIPIENTS {
            return Err("too many recipients");
        }
        self.recipients.push(recipient.clone());
        self.recipient_dsn.push(RcptTo {
            address: recipient,
            dsn: Default::default(),
        });
        Ok(())
    }

    fn accept_mail(&mut self, mail: &MailFrom, dsn: MailDsn) -> Result<bool, &'static str> {
        self.sender = Some(mail.reverse_path.clone());
        self.recipients.clear();
        self.recipient_dsn.clear();
        self.mail_dsn = dsn;
        Ok(false)
    }

    pub fn accept_typed_recipient(&mut self, recipient: RcptTo) -> Result<(), &'static str> {
        self.accept_recipient(recipient.address.clone())?;
        if let Some(accepted) = self.recipient_dsn.last_mut() {
            *accepted = recipient;
        }
        Ok(())
    }

    /// Обновляет typed MAIL DSN параметры после общей policy-проверки MAIL.
    pub fn set_mail_dsn(&mut self, dsn: MailDsn) {
        self.mail_dsn = dsn;
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Transaction запрещает DATA до MAIL и первого RCPT.
    #[test]
    fn transaction_requires_sender_and_recipient() {
        let mut t = Transaction::default();
        assert!(t.apply(&Command::Data).is_err());
        t.apply(&parse("MAIL FROM:<alice@test>\r\n").unwrap())
            .unwrap();
        t.apply(&parse("RCPT TO:<bob@test>\r\n").unwrap()).unwrap();
        assert!(t.apply(&Command::Data).unwrap());
    }

    /// Принятые envelope адреса выдаются ровно один раз при DATA.
    #[test]
    fn data_recipients_are_consumed_once() {
        let mut transaction = Transaction::default();
        transaction
            .apply(&parse("MAIL FROM:<alice@test>\r\n").expect("mail"))
            .expect("sender");
        transaction
            .apply(&parse("RCPT TO:<bob@test>\r\n").expect("recipient"))
            .expect("recipient accepted");
        assert!(transaction.apply(&Command::Data).expect("data"));
        assert_eq!(
            transaction.take_envelope(),
            Envelope {
                reverse_path: "alice@test".into(),
                recipients: vec!["bob@test".into()],
                dsn: DsnEnvelope {
                    reverse_path: "alice@test".into(),
                    mail: MailDsn::default(),
                    recipients: vec![RcptTo {
                        address: "bob@test".into(),
                        dsn: Default::default(),
                    }],
                },
            }
        );
        assert!(transaction.take_envelope().recipients.is_empty());
    }

    /// MAIL и принятые typed RCPT настройки сохраняются в одном ordered envelope.
    #[test]
    fn dsn_parameters_survive_transaction_consumption() {
        let mut transaction = Transaction::default();
        transaction
            .apply(&parse("MAIL FROM:<alice@test> ENVID=id+20x RET=HDRS\r\n").expect("MAIL"))
            .expect("MAIL accepted");
        transaction
            .apply(
                &parse("RCPT TO:<bob@test> NOTIFY=FAILURE ORCPT=rfc822;bob+2Etest\r\n")
                    .expect("RCPT"),
            )
            .expect("RCPT accepted");
        let envelope = transaction.take_envelope();

        assert_eq!(envelope.dsn.reverse_path, "alice@test");
        assert_eq!(
            envelope.dsn.mail.return_content,
            Some(ReturnContent::Headers)
        );
        assert_eq!(envelope.dsn.mail.envelope_id, Some(b"id x".to_vec()));
        let recipient = envelope.dsn.recipients.first().expect("accepted RCPT");
        assert_eq!(envelope.dsn.recipients.len(), 1);
        assert_eq!(recipient.address, "bob@test");
        assert_eq!(
            recipient
                .dsn
                .original_recipient
                .as_ref()
                .map(|value| value.address.clone()),
            Some(b"bob.test".to_vec())
        );
        assert!(matches!(
            recipient.dsn.notify,
            Some(NotifyPolicy::Conditions(ref values)) if values.contains(&NotifyCondition::Failure)
        ));
    }

    /// Повторный MAIL не заменяет отправителя и получателей открытой транзакции.
    #[test]
    fn second_mail_is_rejected_without_changing_open_transaction() {
        let mut transaction = Transaction::default();
        transaction
            .apply(&parse("MAIL FROM:<alice@test>\r\n").expect("first MAIL"))
            .expect("sender accepted");
        transaction
            .apply(&parse("RCPT TO:<bob@test>\r\n").expect("RCPT"))
            .expect("recipient accepted");

        assert!(
            transaction
                .apply(&parse("MAIL FROM:<mallory@test>\r\n").expect("second MAIL"))
                .is_err()
        );
        assert!(!transaction.can_start_mail());
        assert!(
            transaction
                .apply(&Command::Data)
                .expect("original envelope remains")
        );
        let envelope = transaction.take_envelope();
        assert_eq!(envelope.reverse_path, "alice@test");
        assert_eq!(envelope.recipients, vec!["bob@test"]);
    }

    /// Store transaction ограничивает число получателей без роста после отказа.
    #[test]
    fn recipient_limit_is_enforced() {
        let mut transaction = Transaction::default();
        transaction
            .apply(&parse("MAIL FROM:<alice@test>\r\n").expect("mail"))
            .expect("sender");
        for number in 0..MAX_RECIPIENTS {
            transaction
                .accept_recipient(format!("recipient-{number}@test"))
                .expect("within limit");
        }
        assert!(!transaction.has_recipient_capacity());
        assert!(
            transaction
                .accept_recipient("one-more@test".into())
                .is_err()
        );
        assert_eq!(transaction.recipients.len(), MAX_RECIPIENTS);
    }
}
