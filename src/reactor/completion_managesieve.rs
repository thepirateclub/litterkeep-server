//! Завершение ManageSieve операций, возвращённых bounded storage worker-ом.

use super::{Pending, Response, Session, io, managesieve_completion};

/// Итог диспетчеризации отделяет несвязанные протокольные completion-ы.
pub(super) enum Dispatch {
    Handled(io::Result<()>),
    Unhandled(Box<(Pending, Response)>),
}

/// Маршрутизирует только ManageSieve pending/response пары.
pub(super) fn dispatch(session: &mut Session, pending: Pending, response: Response) -> Dispatch {
    match (pending, response) {
        (Pending::ManageSieveAuthenticate { account }, Response::Authenticated { result, .. }) => {
            handled(managesieve_completion::authenticate(
                session, account, result,
            ))
        }
        (Pending::ManageSieveScramLookup, Response::ScramVerifier(response)) => handled(
            managesieve_completion::scram_verifier(session, response.result),
        ),
        (Pending::ManageSieveScramSha256Lookup, Response::ScramSha256Verifier(response)) => {
            handled(managesieve_completion::scram_sha256_verifier(
                session,
                response.result,
            ))
        }
        (Pending::ManageSieveCommand, Response::SieveStored { result, .. }) => {
            handled(managesieve_completion::result(session, result.map(|_| ())))
        }
        (Pending::ManageSieveCommand, Response::SieveMutated { result, .. }) => {
            handled(managesieve_completion::result(session, result))
        }
        (Pending::ManageSieveCommand, Response::SieveSpace { result, .. }) => {
            handled(managesieve_completion::space(session, result))
        }
        (Pending::ManageSieveCommand, Response::SieveScripts { result, .. }) => {
            handled(managesieve_completion::list(session, result))
        }
        (Pending::ManageSieveCommand, Response::SieveSource { result, .. }) => {
            handled(managesieve_completion::source(session, result))
        }
        (pending, response) => Dispatch::Unhandled(Box::new((pending, response))),
    }
}

fn handled(result: io::Result<()>) -> Dispatch {
    Dispatch::Handled(result)
}
