//! Проверки UTF-8 режима и классификации октетов в POP3 maildrop.

use super::message_profile::MessageEncoding;
use super::tests::message;
use super::{Maildrop, Reply, ResponseCode, Session, parse};

/// UTF8 защищает ASCII peer и позволяет передавать корректный raw UTF-8.
#[test]
fn utf8_capability_guards_messages_until_mode_is_enabled() {
    let mut session = Session::default();
    let capabilities = session.apply(parse("CAPA\r\n"));
    assert!(matches!(capabilities, Reply::Multiline(bytes)
        if bytes.windows(11).any(|part| part == b"UTF8 USER\r\n")));
    let _ = session.apply(parse("USER bob\r\n"));
    let _ = session.authenticated(vec![message(
        21,
        b"Subject: caf\xc3\xa9\r\n\r\ncaf\xc3\xa9",
    )]);
    for command in ["LIST 1\r\n", "TOP 1 1\r\n", "RETR 1\r\n"] {
        assert!(matches!(
            session.apply(parse(command)),
            Reply::ErrorCode(ResponseCode::Utf8, _)
        ));
    }

    let mut utf8_session = Session::default();
    utf8_session.tls_established();
    assert!(matches!(
        utf8_session.apply(parse("UTF8\r\n")),
        Reply::Ok(_)
    ));
    assert!(matches!(
        utf8_session.apply(parse("STLS\r\n")),
        Reply::Error(_)
    ));
    let _ = utf8_session.apply(parse("USER bob\r\n"));
    let _ = utf8_session.authenticated(vec![message(
        22,
        b"Subject: caf\xc3\xa9\r\n\r\ncaf\xc3\xa9",
    )]);
    assert!(matches!(utf8_session.apply(parse("RETR 1\r\n")),
        Reply::Message(bytes) if bytes.windows(5).any(|part| part == b"caf\xc3\xa9")));
}

/// LANG публикует английский listing и отклоняет отсутствующие локали.
#[test]
fn lang_lists_selects_and_rejects_unsupported_ranges() {
    let mut session = Session::default();
    let capabilities = session.apply(parse("CAPA\r\n"));
    assert!(matches!(capabilities, Reply::Multiline(bytes)
        if bytes.windows(6).any(|part| part == b"LANG\r\n")));
    assert!(
        matches!(session.apply(parse("LANG\r\n")), Reply::Multiline(bytes)
        if bytes == b"+OK Language listing follows\r\nen English\r\n.\r\n")
    );
    assert!(
        matches!(session.apply(parse("LANG EN\r\n")), Reply::Ok(text)
        if text.starts_with("en "))
    );
    assert!(matches!(session.apply(parse("LANG *\r\n")), Reply::Ok(text)
        if text.starts_with("en ")));
    assert!(matches!(
        session.apply(parse("LANG de\r\n")),
        Reply::Error(_)
    ));
}

/// Профиль различает ASCII, UTF-8, непрозрачные и смешанные октеты.
#[test]
fn maildrop_classifies_ascii_utf8_opaque_and_mixed_bytes() {
    let ascii = Maildrop::new(vec![message(1, b"Subject: plain\r\n\r\nbody")]);
    let unicode = Maildrop::new(vec![message(2, b"Subject: caf\xc3\xa9\r\n\r\nbody")]);
    let malformed = Maildrop::new(vec![message(3, b"binary\xff then caf\xc3\xa9")]);
    let opaque = Maildrop::new(vec![message(4, b"binary\xff only")]);
    assert_eq!(ascii.encoding(None), Ok(MessageEncoding::Ascii));
    assert_eq!(unicode.encoding(Some(1)), Ok(MessageEncoding::Utf8));
    assert_eq!(malformed.encoding(Some(1)), Ok(MessageEncoding::Mixed));
    assert_eq!(opaque.encoding(Some(1)), Ok(MessageEncoding::Opaque));
    assert_eq!(opaque.encoding(None), Ok(MessageEncoding::Opaque));
}

/// Непрозрачные high-bit octets никогда не выдаются за UTF-8.
#[test]
fn session_rejects_opaque_high_bit_octets_in_each_mode() {
    let mut session = Session::default();
    let _ = session.apply(parse("USER bob\r\n"));
    let _ = session.authenticated(vec![message(5, b"Subject: opaque\r\n\r\n\xff")]);
    assert!(matches!(
        session.apply(parse("RETR 1\r\n")),
        Reply::Error("non-ASCII message requires UTF8 mode")
    ));
    let mut utf8_session = Session::default();
    assert!(matches!(
        utf8_session.apply(parse("UTF8\r\n")),
        Reply::Ok(_)
    ));
    let _ = utf8_session.apply(parse("USER bob\r\n"));
    let _ = utf8_session.authenticated(vec![message(6, b"Subject: opaque\r\n\r\n\xff")]);
    assert!(matches!(
        utf8_session.apply(parse("RETR 1\r\n")),
        Reply::Error("message cannot be represented as valid UTF-8")
    ));

    let mut valid_utf8_session = Session::default();
    assert!(matches!(
        valid_utf8_session.apply(parse("UTF8\r\n")),
        Reply::Ok(_)
    ));
    let _ = valid_utf8_session.apply(parse("USER bob\r\n"));
    let _ = valid_utf8_session.authenticated(vec![message(7, b"Subject: caf\xc3\xa9\r\n\r\nbody")]);
    assert!(matches!(
        valid_utf8_session.apply(parse("RETR 1\r\n")),
        Reply::Message(_)
    ));
}
