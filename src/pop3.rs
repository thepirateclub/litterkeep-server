//! POP3 transaction state, shared-store adapter is the next integration step.
#[derive(Default)]
pub struct Session {
    auth: bool,
    deleted: Vec<u32>,
}
pub fn apply(s: &mut Session, line: &str) -> String {
    let mut p = line.split_whitespace();
    let c = p.next().unwrap_or("").to_ascii_uppercase();
    match c.as_str() {
        "USER" if p.next().is_some() => "+OK user accepted\r\n".into(),
        "PASS" if p.next().is_some() => {
            s.auth = true;
            "+OK authenticated\r\n".into()
        }
        "STAT" if s.auth => "+OK 0 0\r\n".into(),
        "UIDL" if s.auth => "+OK\r\n.\r\n".into(),
        "DELE" if s.auth => match p.next().and_then(|x| x.parse().ok()) {
            Some(n) => {
                s.deleted.push(n);
                "+OK marked\r\n".into()
            }
            None => "-ERR invalid message\r\n".into(),
        },
        "RSET" if s.auth => {
            s.deleted.clear();
            "+OK reset\r\n".into()
        }
        "QUIT" => "+OK bye\r\n".into(),
        _ => "-ERR invalid command or state\r\n".into(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auth_is_required() {
        let mut s = Session::default();
        assert!(apply(&mut s, "STAT\r\n").starts_with("-ERR"));
        apply(&mut s, "USER bob\r\n");
        apply(&mut s, "PASS x\r\n");
        assert!(apply(&mut s, "STAT\r\n").starts_with("+OK"));
    }
}
