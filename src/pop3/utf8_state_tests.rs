//! RFC 6856 UTF8-команда разрешена только до USER/AUTH.

use super::{Reply, Session, parse};

/// Отказ на поздний UTF8 не сбрасывает ожидающий USER парольный обмен.
#[test]
fn rejects_utf8_after_user_without_losing_authorization_state() {
    let mut session = Session::default();

    assert!(matches!(session.apply(parse("USER bob\r\n")), Reply::Ok(_)));
    assert!(matches!(
        session.apply(parse("UTF8\r\n")),
        Reply::Error("invalid command for state")
    ));
    assert!(matches!(
        session.apply(parse("PASS test-password\r\n")),
        Reply::Authenticate { username, .. } if username == "bob"
    ));
}
