//! Wire-ответы ManageSieve после typed completion владельца Store.

use super::*;

pub(super) fn authenticate(
    session: &mut Session,
    account: String,
    accepted: bool,
) -> io::Result<()> {
    let authentication_reply =
        session
            .managesieve
            .authenticated(account.clone(), accepted, session.is_encrypted());
    if accepted {
        session.authenticated_account = Some(account);
    }
    reply(session, authentication_reply)
}

pub(super) fn reply(session: &mut Session, reply: managesieve::Reply) -> io::Result<()> {
    match reply {
        managesieve::Reply::Text(bytes) => session.push(&bytes),
        _ => session.push(b"NO \"state failure\"\r\n"),
    }
}
pub(super) fn result(session: &mut Session, result: io::Result<()>) -> io::Result<()> {
    match result {
        Ok(()) => session.push(b"OK \"completed\"\r\n"),
        Err(_) => session.push(b"NO \"operation failed\"\r\n"),
    }
}
pub(super) fn list(
    session: &mut Session,
    result: io::Result<Vec<crate::store::ScriptInfo>>,
) -> io::Result<()> {
    match result {
        Ok(scripts) => {
            for script in scripts {
                let active = match script.active {
                    true => " ACTIVE",
                    false => "",
                };
                session.push(format!("\"{}\"{}\r\n", script.name, active).as_bytes())?;
            }
            session.push(b"OK \"list completed\"\r\n")
        }
        Err(_) => session.push(b"NO \"list failed\"\r\n"),
    }
}
pub(super) fn source(session: &mut Session, result: io::Result<(u64, String)>) -> io::Result<()> {
    match result {
        Ok((_, source)) => session.push(
            format!(
                "{{{}}}\r\n{}\r\nOK \"get completed\"\r\n",
                source.len(),
                source
            )
            .as_bytes(),
        ),
        Err(_) => session.push(b"NO \"script not found\"\r\n"),
    }
}
