//! SMTP submission transaction state.
#[path = "smtp/command.rs"]
mod command;
pub use command::{Command, MAX_MESSAGE_SIZE, MailFrom, ParseError, parse};

/// Проверенный envelope, передаваемый из SMTP FSM в локальную delivery.
#[derive(Debug, Eq, PartialEq)]
pub struct Envelope {
    pub reverse_path: String,
    pub recipients: Vec<String>,
}
/// Верхняя граница получателей одного SMTP envelope в локальном профиле.
const MAX_RECIPIENTS: usize = 100;
#[derive(Default, Debug)]
pub struct Transaction {
    sender: Option<String>,
    recipients: Vec<String>,
}
impl Transaction {
    pub fn apply(&mut self, command: &Command) -> Result<bool, &'static str> {
        match command {
            Command::Mail(mail) => {
                self.sender = Some(mail.reverse_path.clone());
                self.recipients.clear();
                Ok(false)
            }
            Command::Rcpt(r) if self.sender.is_some() => {
                self.accept_recipient(r.clone())?;
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
        Envelope {
            reverse_path: self.sender.take().unwrap_or_default(),
            recipients: std::mem::take(&mut self.recipients),
        }
    }

    /// Проверяет, можно ли принять очередного RCPT в текущем envelope.
    pub fn can_accept_recipient(&self) -> bool {
        self.sender.is_some()
    }

    /// Сообщает, начата ли envelope-транзакция командой MAIL.
    pub fn has_sender(&self) -> bool {
        self.sender.is_some()
    }

    /// Добавляет проверенный policy слоем получатель в активный envelope.
    pub fn accept_recipient(&mut self, recipient: String) -> Result<(), &'static str> {
        if !self.can_accept_recipient() {
            return Err("bad command sequence");
        }
        if self.recipients.len() >= MAX_RECIPIENTS {
            return Err("too many recipients");
        }
        self.recipients.push(recipient);
        Ok(())
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
            }
        );
        assert!(transaction.take_envelope().recipients.is_empty());
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
        assert!(
            transaction
                .accept_recipient("one-more@test".into())
                .is_err()
        );
    }
}
