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

/// Преобразует worker completion в первый SCRAM challenge ровно один раз.
pub(super) fn scram_verifier(
    session: &mut Session,
    result: io::Result<Option<crate::sasl::scram_sha1_record::ScramSha1Record>>,
) -> io::Result<()> {
    match result {
        Ok(Some(credential)) => match session.managesieve.scram_server_first(credential) {
            Ok(protocol_reply) => reply(session, protocol_reply),
            Err(_) => authentication_failure(session, false),
        },
        Ok(None) => authentication_failure(session, true),
        Err(_) => authentication_failure(session, false),
    }
}

/// Кодирует результат RFC 5804 HAVESPACE без смешения с I/O completion.
pub(super) fn space(session: &mut Session, result: io::Result<bool>) -> io::Result<()> {
    match result {
        Ok(true) => session.push(b"OK \"space available\"\r\n"),
        Ok(false) => session.push(b"NO (QUOTA) \"space unavailable\"\r\n"),
        Err(_) => session.push(b"NO \"storage failure\"\r\n"),
    }
}

/// Не раскрывает отсутствие аккаунта, сохраняя отдельный отказ файлового слоя.
fn authentication_failure(session: &mut Session, rejected: bool) -> io::Result<()> {
    session.managesieve.scram_failed();
    match rejected {
        true => session.push(b"NO \"authentication failed\"\r\n"),
        false => session.push(b"NO \"authentication temporarily unavailable\"\r\n"),
    }
}

pub(super) fn reply(session: &mut Session, reply: managesieve::Reply) -> io::Result<()> {
    match reply {
        managesieve::Reply::Text(bytes) => session.push(&bytes),
        managesieve::Reply::ScramChallenge { response } => {
            session.push(format!("\"{response}\"\r\n").as_bytes())
        }
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
                let name = super::managesieve_string::quoted(&script.name);
                session.push(format!("{name}{active}\r\n").as_bytes())?;
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

    /// LISTSCRIPTS сохраняет допустимые специальные символы как одну строку.
    #[test]
    fn list_escapes_script_names_and_keeps_active_marker_outside_string() {
        let (mut session, _peer) = session();
        let scripts = vec![crate::store::ScriptInfo {
            name: "review\"\\draft/папка".into(),
            version: 1,
            active: true,
        }];

        list(&mut session, Ok(scripts)).expect("LISTSCRIPTS response");

        assert_eq!(
            session.output,
            "\"review\\\"\\\\draft/папка\" ACTIVE\r\nOK \"list completed\"\r\n".as_bytes()
        );
    }

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

    fn session() -> (Session, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let peer = TcpStream::connect(listener.local_addr().expect("address")).expect("peer");
        let (stream, _) = listener.accept().expect("accept");
        let mut session =
            Session::new(Kind::ManageSieve, stream, 1, None, None).expect("ManageSieve session");
        session.output.clear();
        (session, peer)
    }
}
