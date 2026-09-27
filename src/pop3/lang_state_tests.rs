//! RFC 6856 LANG command state regression.

use super::{Reply, Session, parse};

/// LANG запрещён между USER и PASS, но разрешён в AUTHORIZATION и TRANSACTION.
#[test]
fn rejects_lang_after_user_and_preserves_authentication_exchange() {
    let mut session = Session::default();

    assert!(matches!(session.apply(parse("LANG EN\r\n")), Reply::Ok(_)));
    assert!(matches!(session.apply(parse("USER bob\r\n")), Reply::Ok(_)));
    assert!(matches!(
        session.apply(parse("LANG EN\r\n")),
        Reply::Error("invalid command for state")
    ));
    assert!(matches!(
        session.apply(parse("PASS test-password\r\n")),
        Reply::Authenticate { username, .. } if username == "bob"
    ));
    let _ = session.authenticated(Vec::new());
    assert!(matches!(session.apply(parse("LANG EN\r\n")), Reply::Ok(_)));
}
