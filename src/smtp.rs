//! RFC 5321 command parsing and submission transaction state.
#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Ehlo(String),
    Helo(String),
    Mail(String),
    Rcpt(String),
    Data,
    /// Initial response механизма SASL PLAIN в base64-представлении.
    AuthPlain(String),
    StartTls,
    Noop,
    Rset,
    Quit,
}

/// Проверенный envelope, передаваемый из SMTP FSM в локальную delivery.
#[derive(Debug, Eq, PartialEq)]
pub struct Envelope {
    pub reverse_path: String,
    pub recipients: Vec<String>,
}
/// Верхняя граница получателей одного SMTP envelope в локальном профиле.
const MAX_RECIPIENTS: usize = 100;
pub fn parse(line: &str) -> Result<Command, &'static str> {
    let (verb, arg) = line
        .trim_end_matches("\r\n")
        .split_once(' ')
        .unwrap_or((line.trim_end_matches("\r\n"), ""));
    match verb.to_ascii_uppercase().as_str() {
        "EHLO" if !arg.is_empty() => Ok(Command::Ehlo(arg.into())),
        "HELO" if !arg.is_empty() => Ok(Command::Helo(arg.into())),
        "MAIL" => address(arg, "FROM:", true).map(Command::Mail),
        "RCPT" => address(arg, "TO:", false).map(Command::Rcpt),
        "DATA" if arg.is_empty() => Ok(Command::Data),
        "AUTH" => parse_auth(arg),
        "STARTTLS" if arg.is_empty() => Ok(Command::StartTls),
        "NOOP" if arg.is_empty() => Ok(Command::Noop),
        "RSET" if arg.is_empty() => Ok(Command::Rset),
        "QUIT" if arg.is_empty() => Ok(Command::Quit),
        _ => Err("unrecognized or malformed command"),
    }
}

/// Распознаёт только реализованный SASL-механизм, не рекламируя прочие.
fn parse_auth(argument: &str) -> Result<Command, &'static str> {
    let Some((mechanism, response)) = argument.split_once(' ') else {
        return Err("missing SASL initial response");
    };
    match mechanism.to_ascii_uppercase().as_str() {
        "PLAIN" if !response.is_empty() => Ok(Command::AuthPlain(response.to_owned())),
        _ => Err("unsupported SASL mechanism"),
    }
}
fn address(arg: &str, prefix: &str, accepts_empty_path: bool) -> Result<String, &'static str> {
    let (actual, rest) = arg.split_at(
        arg.find(':')
            .map(|index| index + 1)
            .ok_or("missing envelope prefix")?,
    );
    if !actual.eq_ignore_ascii_case(prefix) {
        return Err("missing envelope prefix");
    }
    let value = rest.trim();
    let Some(path) = value.strip_prefix('<') else {
        return Err("invalid path");
    };
    let Some((mailbox, suffix)) = path.split_once('>') else {
        return Err("invalid path");
    };
    if let (true, false) = (mailbox.is_empty(), accepts_empty_path) {
        return Err("invalid path");
    }
    let has_parameter_separator = suffix.chars().next().is_some_and(char::is_whitespace);
    match suffix.is_empty() {
        true => Ok(mailbox.into()),
        false => match has_parameter_separator {
            true => Ok(mailbox.into()),
            false => Err("invalid path"),
        },
    }
}
#[derive(Default, Debug)]
pub struct Transaction {
    sender: Option<String>,
    recipients: Vec<String>,
}
impl Transaction {
    pub fn apply(&mut self, command: &Command) -> Result<bool, &'static str> {
        match command {
            Command::Mail(s) => {
                self.sender = Some(s.clone());
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
    #[test]
    fn transaction_requires_sender_and_recipient() {
        let mut t = Transaction::default();
        assert!(t.apply(&Command::Data).is_err());
        t.apply(&parse("MAIL FROM:<alice@test>\r\n").unwrap())
            .unwrap();
        t.apply(&parse("RCPT TO:<bob@test>\r\n").unwrap()).unwrap();
        assert!(t.apply(&Command::Data).unwrap());
    }

    #[test]
    fn accepts_standard_client_mail_size_parameter() {
        assert_eq!(
            parse("mail FROM:<alice@test> size=37\r\n").expect("standard MAIL"),
            Command::Mail("alice@test".into())
        );
    }

    #[test]
    fn accepts_empty_mta_reverse_path_and_noop() {
        assert_eq!(
            parse("MAIL FROM:<>\r\n").expect("empty reverse path"),
            Command::Mail(String::new())
        );
        assert_eq!(parse("NOOP\r\n").expect("noop"), Command::Noop);
        assert_eq!(
            parse("HELO sender.test\r\n").expect("helo"),
            Command::Helo("sender.test".into())
        );
    }

    #[test]
    fn parses_plain_auth_initial_response() {
        assert_eq!(
            parse("AUTH PLAIN AGJvYgB0ZXN0LXBhc3N3b3Jk\r\n").expect("plain auth"),
            Command::AuthPlain("AGJvYgB0ZXN0LXBhc3N3b3Jk".into())
        );
    }

    #[test]
    fn parses_starttls_without_arguments() {
        assert_eq!(parse("STARTTLS\r\n").expect("starttls"), Command::StartTls);
        assert!(parse("STARTTLS now\r\n").is_err());
    }

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
