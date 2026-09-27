//! RFC 6856 LANG valid in AUTHORIZATION and TRANSACTION RFC states.

use super::{Reply, Session, parse};

/// USERAccepted остаётся AUTHORIZATION, поэтому LANG допустим до и после USER.
#[test]
fn accepts_lang_through_user_pass_exchange_and_transaction() {
    let mut session = Session::default();

    assert!(matches!(session.apply(parse("LANG EN\r\n")), Reply::Ok(_)));
    assert!(matches!(session.apply(parse("USER bob\r\n")), Reply::Ok(_)));
    assert!(matches!(
        session.apply(parse("LANG EN\r\n")),
        Reply::Ok(text) if text == "en Language selected"
    ));
    assert!(matches!(
        session.apply(parse("PASS test-password\r\n")),
        Reply::Authenticate { username, .. } if username == "bob"
    ));
    let _ = session.authenticated(Vec::new());
    assert!(matches!(session.apply(parse("LANG EN\r\n")), Reply::Ok(_)));
}
