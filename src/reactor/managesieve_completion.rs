//! Wire-ответы ManageSieve после typed completion владельца Store.

use super::*;

pub(super) fn authenticate(
    session: &mut Session,
    account: String,
    result: io::Result<bool>,
) -> io::Result<()> {
    let accepted = match result {
        Ok(accepted) => accepted,
        Err(_) => return session.push(b"NO \"authentication temporarily unavailable\"\r\n"),
    };
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
    session.push(result_response(result))
}

fn result_response(result: io::Result<()>) -> &'static [u8] {
    match result {
        Ok(()) => b"OK \"completed\"\r\n",
        Err(error) if crate::store::is_sieve_quota_error(&error) => {
            b"NO (QUOTA) \"quota exceeded\"\r\n"
        }
        Err(error) if crate::store::is_sieve_missing_error(&error) => {
            b"NO (NONEXISTENT) \"script does not exist\"\r\n"
        }
        Err(error) if crate::store::is_sieve_active_error(&error) => {
            b"NO (ACTIVE) \"active script cannot be deleted\"\r\n"
        }
        Err(error) if crate::store::is_sieve_already_exists_error(&error) => {
            b"NO (ALREADYEXISTS) \"script already exists\"\r\n"
        }
        Err(_) => b"NO \"operation failed\"\r\n",
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
        Err(error) if crate::store::is_sieve_missing_error(&error) => {
            session.push(b"NO (NONEXISTENT) \"script does not exist\"\r\n")
        }
        Err(_) => session.push(b"NO \"script not found\"\r\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Квотные отказы имеют машинно-читаемый RFC код, прочие ошибки не маскируются.
    #[test]
    fn mutation_result_distinguishes_quota_from_other_failures() {
        let quota = Err(crate::store::sieve_quota_test_error());
        let missing = Err(crate::store::sieve_missing_test_error());
        let active = Err(crate::store::sieve_active_test_error());
        let already_exists = Err(crate::store::sieve_already_exists_test_error());
        let storage = Err(io::Error::new(io::ErrorKind::StorageFull, "disk full"));
        assert_eq!(result_response(Ok(())), b"OK \"completed\"\r\n");
        assert_eq!(result_response(quota), b"NO (QUOTA) \"quota exceeded\"\r\n");
        assert_eq!(
            result_response(missing),
            b"NO (NONEXISTENT) \"script does not exist\"\r\n"
        );
        assert_eq!(
            result_response(active),
            b"NO (ACTIVE) \"active script cannot be deleted\"\r\n"
        );
        assert_eq!(
            result_response(already_exists),
            b"NO (ALREADYEXISTS) \"script already exists\"\r\n"
        );
        assert_eq!(result_response(storage), b"NO \"operation failed\"\r\n");
    }
}
