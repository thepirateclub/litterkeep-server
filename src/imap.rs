//! Minimal tagged IMAP command parser and authentication/select state.
#[derive(Default, Debug)]
pub struct Session {
    logged_in: bool,
    selected: Option<String>,
}
pub fn apply(session: &mut Session, line: &str) -> String {
    let mut p = line.split_whitespace();
    let tag = p.next().unwrap_or("*");
    let command = p.next().unwrap_or("").to_ascii_uppercase();
    match command.as_str() {
        "LOGIN" if p.next().is_some() && p.next().is_some() => {
            session.logged_in = true;
            format!("{tag} OK LOGIN completed\r\n")
        }
        "SELECT" if session.logged_in => {
            let box_name = p.next().unwrap_or("INBOX");
            session.selected = Some(box_name.into());
            format!("* 0 EXISTS\r\n{tag} OK [READ-WRITE] SELECT completed\r\n")
        }
        "FETCH" if session.selected.is_some() => format!("{tag} OK FETCH completed\r\n"),
        "LOGOUT" => format!("* BYE logging out\r\n{tag} OK LOGOUT completed\r\n"),
        _ => format!("{tag} BAD invalid command or state\r\n"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requires_login_before_select() {
        let mut s = Session::default();
        assert!(apply(&mut s, "a SELECT INBOX\r\n").contains("BAD"));
        assert!(apply(&mut s, "a LOGIN bob secret\r\n").contains("OK"));
        assert!(apply(&mut s, "b SELECT INBOX\r\n").contains("EXISTS"));
    }
}
