//! RFC 5321 command parsing and submission transaction state.
#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Ehlo(String),
    Mail(String),
    Rcpt(String),
    Data,
    Rset,
    Quit,
}
pub fn parse(line: &str) -> Result<Command, &'static str> {
    let (verb, arg) = line
        .trim_end_matches("\r\n")
        .split_once(' ')
        .unwrap_or((line.trim_end_matches("\r\n"), ""));
    match verb.to_ascii_uppercase().as_str() {
        "EHLO" | "HELO" if !arg.is_empty() => Ok(Command::Ehlo(arg.into())),
        "MAIL" => address(arg, "FROM:").map(Command::Mail),
        "RCPT" => address(arg, "TO:").map(Command::Rcpt),
        "DATA" if arg.is_empty() => Ok(Command::Data),
        "RSET" if arg.is_empty() => Ok(Command::Rset),
        "QUIT" if arg.is_empty() => Ok(Command::Quit),
        _ => Err("unrecognized or malformed command"),
    }
}
fn address(arg: &str, prefix: &str) -> Result<String, &'static str> {
    let (actual, rest) = arg.split_at(
        arg.find(':')
            .map(|index| index + 1)
            .ok_or("missing envelope prefix")?,
    );
    if !actual.eq_ignore_ascii_case(prefix) {
        return Err("missing envelope prefix");
    }
    let value = rest.trim();
    if let Some(end) = value.find('>')
        && value.starts_with('<')
        && end > 1
        && (end + 1 == value.len() || value.as_bytes()[end + 1].is_ascii_whitespace())
    {
        Ok(value[1..end].into())
    } else {
        Err("invalid path")
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
                self.recipients.push(r.clone());
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
}
