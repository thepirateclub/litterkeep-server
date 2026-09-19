//! Типизированная POP3 FSM без доступа к ящику до аутентификации.

#[derive(Default)]
enum Phase {
    #[default]
    Authorization,
    Transaction,
    Update,
}
pub enum Command {
    User(String),
    Pass(String),
    Stat,
    Uidl,
    Dele(u32),
    Rset,
    Quit,
    Invalid,
}
pub enum Reply {
    Ok(String),
    Error,
}

/// Состояние POP3 сессии; worker позднее переводит её в Transaction после PASS.
#[derive(Default)]
pub struct Session {
    phase: Phase,
    username: Option<String>,
    deleted: Vec<u32>,
}

impl Session {
    /// Выполняет уже типизированную команду без фиктивного успеха пароля.
    pub fn apply(&mut self, command: Command) -> Reply {
        match (&self.phase, command) {
            (Phase::Authorization, Command::User(name)) => {
                self.username = Some(name);
                Reply::Ok("user accepted".into())
            }
            (Phase::Authorization, Command::Pass(password)) => {
                match (self.username.as_ref(), password.is_empty()) {
                    (Some(_), false) => Reply::Error,
                    _ => Reply::Error,
                }
            }
            (Phase::Transaction, Command::Stat) => Reply::Ok("0 0".into()),
            (Phase::Transaction, Command::Uidl) => Reply::Ok("\r\n.".into()),
            (Phase::Transaction, Command::Dele(number)) => {
                self.deleted.push(number);
                Reply::Ok("marked".into())
            }
            (Phase::Transaction, Command::Rset) => {
                self.deleted.clear();
                Reply::Ok("reset".into())
            }
            (_, Command::Quit) => {
                self.phase = Phase::Update;
                Reply::Ok("bye".into())
            }
            _ => Reply::Error,
        }
    }

    /// Только completion worker-а после Argon2-проверки открывает TRANSACTION.
    pub fn authenticate(&mut self, accepted: bool) -> Reply {
        match (self.username.as_ref(), accepted) {
            (Some(_), true) => {
                self.phase = Phase::Transaction;
                Reply::Ok("authenticated".into())
            }
            _ => Reply::Error,
        }
    }
}

/// Единственная граница строковых POP3 verbs.
pub fn parse(line: &str) -> Command {
    let mut words = line.split_whitespace();
    match words
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase()
        .as_str()
    {
        "USER" => words
            .next()
            .map_or(Command::Invalid, |value| Command::User(value.into())),
        "PASS" => words
            .next()
            .map_or(Command::Invalid, |value| Command::Pass(value.into())),
        "STAT" => Command::Stat,
        "UIDL" => Command::Uidl,
        "DELE" => words
            .next()
            .and_then(|value| value.parse().ok())
            .map_or(Command::Invalid, Command::Dele),
        "RSET" => Command::Rset,
        "QUIT" => Command::Quit,
        _ => Command::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// До worker-проверки PASS сессия не получает доступ к TRANSACTION.
    #[test]
    fn auth_is_required() {
        let mut session = Session::default();
        assert!(matches!(session.apply(parse("STAT\r\n")), Reply::Error));
        assert!(matches!(session.apply(parse("USER bob\r\n")), Reply::Ok(_)));
        assert!(matches!(session.apply(parse("PASS x\r\n")), Reply::Error));
        assert!(matches!(session.authenticate(true), Reply::Ok(_)));
        assert!(matches!(session.apply(parse("STAT\r\n")), Reply::Ok(_)));
    }
}
