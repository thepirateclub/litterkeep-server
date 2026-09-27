//! Проверки допустимых состояний команды NOOP по RFC 1939.

use super::{Reply, Session, parse};

/// NOOP отклоняется до входа и принимается в TRANSACTION без потери USER.
#[test]
fn noop_is_limited_to_transaction_state() {
    let mut session = Session::default();

    assert!(matches!(
        session.apply(parse("NOOP\r\n")),
        Reply::Error("invalid command for state")
    ));
    assert!(matches!(session.apply(parse("USER bob\r\n")), Reply::Ok(_)));
    assert!(matches!(
        session.apply(parse("NOOP\r\n")),
        Reply::Error("invalid command for state")
    ));
    assert!(matches!(
        session.apply(parse("PASS secret\r\n")),
        Reply::Authenticate { username, .. } if username == "bob"
    ));
    let _ = session.authenticated(Vec::new());
    assert!(matches!(
        session.apply(parse("NOOP\r\n")),
        Reply::Ok(message) if message == "noop"
    ));
}
