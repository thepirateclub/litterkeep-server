//! Проверяет RFC 1939 listing arity и размеры нормализованного сообщения.

use super::*;

#[test]
fn list_and_uidl_with_message_number_use_single_line_responses() {
    let mut session = Session::default();
    let _ = session.apply(parse("USER bob\r\n"));
    let _ = session.authenticated(vec![message(9, b"Subject: x\n\n.leading\r\n")]);
    assert!(matches!(
        session.apply(parse("LIST 1\r\n")),
        Reply::Ok(text) if text == "1 24"
    ));
    assert!(matches!(
        session.apply(parse("UIDL 1\r\n")),
        Reply::Ok(text) if text == "1 lk-0000000000000009"
    ));
    assert!(matches!(
        session.apply(parse("LIST\r\n")),
        Reply::Multiline(bytes) if bytes.windows(6).any(|line| line == b"1 24\r\n")
    ));
    assert!(matches!(
        session.apply(parse("UIDL\r\n")),
        Reply::Multiline(bytes) if bytes.windows(23).any(|line| line == b"1 lk-0000000000000009\r\n")
    ));
}

#[test]
fn list_size_counts_crlf_without_dot_stuffing() {
    let drop = Maildrop::new(vec![message(1, b"Subject: x\n\n.leading\r\n")]);
    assert_eq!(drop.stat(), (1, 24));
    assert_eq!(drop.list(Some(1)), Ok(vec![(1, 24)]));
    assert_eq!(
        drop.retrieve(1),
        Ok(b"Subject: x\r\n\r\n..leading\r\n.\r\n".to_vec())
    );
}
