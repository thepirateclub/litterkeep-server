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

#[test]
fn list_size_preserves_carriage_returns_not_followed_by_line_feed() {
    let drop = Maildrop::new(vec![message(2, b"a\rX\nlast\r")]);

    assert_eq!(drop.list(Some(1)), Ok(vec![(1, 12)]));
    assert_eq!(drop.retrieve(1), Ok(b"a\rX\r\nlast\r\r\n.\r\n".to_vec()));
}

#[test]
fn deleted_messages_are_hidden_from_stat_list_and_uidl_until_rset() {
    let mut session = Session::default();
    let _ = session.apply(parse("USER bob\r\n"));
    let _ = session.authenticated(vec![
        message(9, b"Subject: first\r\n\r\nbody"),
        message(10, b"Subject: second\r\n\r\nbody"),
    ]);
    assert!(matches!(session.apply(parse("DELE 1\r\n")), Reply::Ok(_)));
    assert!(
        matches!(session.apply(parse("STAT\r\n")), Reply::Ok(value) if value.starts_with("1 "))
    );
    let listing = match session.apply(parse("LIST\r\n")) {
        Reply::Multiline(value) => value,
        _ => panic!("LIST without a number must be multiline"),
    };
    assert!(
        listing.windows(6).any(|line| line == b"2 25\r\n")
            && !listing.windows(6).any(|line| line == b"1 22\r\n"),
        "deleted listing was not filtered: {listing:?}"
    );
    assert!(
        matches!(session.apply(parse("UIDL\r\n")), Reply::Multiline(value)
        if value.windows(23).any(|line| line == b"2 lk-000000000000000a\r\n")
            && !value.windows(23).any(|line| line == b"1 lk-0000000000000009\r\n"))
    );
    assert!(matches!(
        session.apply(parse("LIST 1\r\n")),
        Reply::Error(_)
    ));
    assert!(matches!(
        session.apply(parse("UIDL 1\r\n")),
        Reply::Error(_)
    ));
    assert!(matches!(session.apply(parse("RSET\r\n")), Reply::Ok(_)));
    assert!(
        matches!(session.apply(parse("STAT\r\n")), Reply::Ok(value) if value.starts_with("2 "))
    );
}
