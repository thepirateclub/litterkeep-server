//! RFC 6856 UTF8 действует на всю POP3 AUTHORIZATION state.

use super::{Reply, Session, parse};

/// USERAccepted остаётся AUTHORIZATION, поэтому UTF8 не отменяет PASS.
#[test]
fn accepts_utf8_after_user_and_preserves_pass_exchange() {
    let mut session = Session::default();

    assert!(matches!(session.apply(parse("USER bob\r\n")), Reply::Ok(_)));
    assert!(matches!(
        session.apply(parse("UTF8\r\n")),
        Reply::Ok(text) if text == "UTF-8 mode enabled"
    ));
    assert!(matches!(
        session.apply(parse("PASS test-password\r\n")),
        Reply::Authenticate { username, .. } if username == "bob"
    ));
}
