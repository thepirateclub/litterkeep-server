//! Проверяет запрет MIME binary CTE при POP3 передаче сообщений.

use super::*;

#[test]
fn pop3_rejects_binary_cte_on_top_and_retr_even_in_utf8_mode() {
    let raw = b"From: alice@test\r\nTo: bob@test\r\nSubject: binary CTE policy\r\nMIME-Version: 1.0\r\nContent-Type: multipart/report; boundary=policy\r\n\r\n--policy\r\nContent-Type: text/plain\r\n\r\nnotice\r\n--policy\r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: binary\r\n\r\nopaque\x00octets\r\n--policy--\r\n";
    let mut session = Session::default();
    assert!(matches!(session.apply(parse("UTF8\r\n")), Reply::Ok(_)));
    let _ = session.apply(parse("USER bob\r\n"));
    let _ = session.authenticated(vec![message(31, raw)]);
    assert!(matches!(
        session.apply(parse("TOP 1 2\r\n")),
        Reply::Error("MIME binary transfer encoding is not permitted")
    ));
    assert!(matches!(
        session.apply(parse("RETR 1\r\n")),
        Reply::Error("MIME binary transfer encoding is not permitted")
    ));
}

#[test]
fn pop3_rejects_unverifiable_mime_structure_instead_of_assuming_safe_cte() {
    let raw = b"From: alice@test\r\nTo: bob@test\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=broken\r\n\r\n--broken\r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: binary\r\n\r\nopaque\0octets\r\n";
    let mut session = Session::default();
    assert!(matches!(session.apply(parse("UTF8\r\n")), Reply::Ok(_)));
    let _ = session.apply(parse("USER bob\r\n"));
    let _ = session.authenticated(vec![message(32, raw)]);

    for reply in [
        session.apply(parse("TOP 1 2\r\n")),
        session.apply(parse("RETR 1\r\n")),
    ] {
        assert!(matches!(
            reply,
            Reply::Error("MIME structure cannot be verified for POP3")
        ));
    }
}
