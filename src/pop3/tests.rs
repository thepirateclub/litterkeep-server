//! Проверки POP3 FSM, snapshot и отложенного удаления.

use super::maildrop::MessageEncoding;
use super::*;
use crate::{mail_date::MailDate, mail_flags::MessageFlags, store::Message};
use std::path::PathBuf;

fn message(uid: u64, bytes: &[u8]) -> Message {
    Message {
        uid,
        path: PathBuf::new(),
        bytes: bytes.to_vec(),
        flags: MessageFlags::default(),
        keywords: std::collections::BTreeSet::new(),
        internal_date: MailDate::from_unix_seconds(0),
    }
}

#[test]
fn authentication_and_stat_require_worker_completion() {
    let mut session = Session::default();
    assert!(matches!(session.apply(parse("STAT\r\n")), Reply::Error(_)));
    assert!(matches!(session.apply(parse("USER pop\r\n")), Reply::Ok(_)));
    assert!(matches!(
        session.apply(parse("PASS test password\r\n")),
        Reply::Authenticate { password, .. } if password == b"test password"
    ));
    assert!(matches!(
        session.authenticated(vec![message(7, b"Subject: one\r\n\r\nbody")]),
        Reply::Ok(_)
    ));
    assert!(
        matches!(session.apply(parse("STAT\r\n")), Reply::Ok(value) if value.starts_with("1 "))
    );
}

#[test]
fn user_pass_exchange_rejects_repeated_user_and_resets_after_failure() {
    let mut session = Session::default();
    assert!(matches!(session.apply(parse("USER pop\r\n")), Reply::Ok(_)));
    assert!(matches!(
        session.apply(parse("USER alice\r\n")),
        Reply::Error(_)
    ));
    assert!(matches!(
        session.apply(parse("PASS wrong\r\n")),
        Reply::Error(_)
    ));
    assert!(matches!(session.apply(parse("USER pop\r\n")), Reply::Ok(_)));
    assert!(matches!(
        session.apply(parse("PASS wrong\r\n")),
        Reply::Authenticate { username, .. } if username == "pop"
    ));
    assert!(matches!(session.authentication_rejected(), Reply::Error(_)));
    assert!(matches!(
        session.apply(parse("PASS retry\r\n")),
        Reply::Error(_)
    ));
    assert!(matches!(
        session.apply(parse("USER alice\r\n")),
        Reply::Ok(_)
    ));
    assert!(matches!(
        session.apply(parse("PASS correct\r\n")),
        Reply::Authenticate { username, .. } if username == "alice"
    ));
}

#[test]
fn capa_advertises_stls_before_password_exchange() {
    let mut session = Session::default();
    assert!(
        matches!(session.apply(parse("CAPA\r\n")), Reply::Multiline(bytes) if bytes.windows(4).any(|part| part == b"STLS"))
    );
    assert!(matches!(
        session.apply(parse("CAPA\r\n")),
        Reply::Multiline(bytes) if !bytes.windows(10).any(|part| part == b"SASL PLAIN")
    ));
    session.tls_established();
    assert!(matches!(
        session.apply(parse("CAPA\r\n")),
        Reply::Multiline(bytes) if bytes.windows(10).any(|part| part == b"SASL PLAIN")
    ));
    assert!(matches!(
        session.apply(parse("CAPA\r\n")),
        Reply::Multiline(bytes) if !bytes.windows(4).any(|part| part == b"STLS")
    ));
}

#[test]
fn sasl_plain_requires_tls_and_authenticates_without_user_pass() {
    let response = crate::sasl::encode_plain("bob", b"secret");
    let mut session = Session::default();
    let command = parse(&format!("AUTH PLAIN {response}\r\n"));
    assert!(matches!(session.apply(command), Reply::Error(_)));

    session.tls_established();
    let command = parse(&format!("AUTH PLAIN {response}\r\n"));
    assert!(matches!(
        session.apply(command),
        Reply::Authenticate { username, password }
            if username == "bob" && password == b"secret"
    ));
}

#[test]
fn sasl_plain_continuation_supports_response_and_cancellation() {
    let response = crate::sasl::encode_plain("bob", b"secret");
    let mut session = Session::default();
    session.tls_established();
    assert!(matches!(
        session.apply(parse("USER alice\r\n")),
        Reply::Ok(_)
    ));
    assert!(matches!(
        session.apply(parse("AUTH PLAIN\r\n")),
        Reply::SaslContinue
    ));
    assert!(session.awaiting_sasl_response());
    assert!(matches!(session.sasl_plain_response("*"), Reply::Error(_)));
    assert!(!session.awaiting_sasl_response());
    assert!(matches!(
        session.apply(parse("PASS correct\r\n")),
        Reply::Authenticate { username, .. } if username == "alice"
    ));
    assert!(matches!(session.authentication_rejected(), Reply::Error(_)));
    assert!(matches!(
        session.apply(parse("AUTH PLAIN\r\n")),
        Reply::SaslContinue
    ));
    assert!(matches!(
        session.sasl_plain_response(&response),
        Reply::Authenticate { username, password }
            if username == "bob" && password == b"secret"
    ));
}

#[test]
fn failed_sasl_auth_preserves_prior_authorization_and_allows_retry() {
    let mut session = Session::default();
    session.tls_established();
    assert!(matches!(
        session.apply(parse("USER alice\r\n")),
        Reply::Ok(_)
    ));
    assert!(matches!(
        session.apply(parse("AUTH PLAIN invalid\r\n")),
        Reply::Error(_)
    ));
    assert!(matches!(
        session.apply(parse("PASS correct\r\n")),
        Reply::Authenticate { username, .. } if username == "alice"
    ));
    assert!(matches!(session.authentication_rejected(), Reply::Error(_)));
    assert!(matches!(session.apply(parse("USER bob\r\n")), Reply::Ok(_)));
}

#[test]
fn failed_sasl_backend_auth_allows_new_auth_exchange() {
    let response = crate::sasl::encode_plain("bob", b"wrong");
    let mut session = Session::default();
    session.tls_established();
    assert!(matches!(
        session.apply(parse(&format!("AUTH PLAIN {response}\r\n"))),
        Reply::Authenticate { .. }
    ));
    assert!(matches!(session.authentication_rejected(), Reply::Error(_)));
    let response = crate::sasl::encode_plain("alice", b"correct");
    assert!(matches!(
        session.apply(parse(&format!("AUTH PLAIN {response}\r\n"))),
        Reply::Authenticate { username, .. } if username == "alice"
    ));
}

#[test]
fn empty_sasl_initial_response_is_not_confused_with_base64_data() {
    let mut session = Session::default();
    session.tls_established();
    let _ = session.apply(parse("USER bob\r\n"));
    assert!(matches!(
        session.apply(parse("AUTH PLAIN =\r\n")),
        Reply::Error(_)
    ));
    assert!(matches!(
        session.apply(parse("PASS test-password\r\n")),
        Reply::Authenticate { username, .. } if username == "bob"
    ));
}

#[test]
fn pop3_utf8_capability_guards_messages_until_mode_is_enabled() {
    let mut session = Session::default();
    let capabilities = session.apply(parse("CAPA\r\n"));
    assert!(
        matches!(capabilities, Reply::Multiline(bytes) if bytes.windows(6).any(|part| part == b"UTF8\r\n"))
    );
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
    assert!(matches!(
        utf8_session.apply(parse("RETR 1\r\n")),
        Reply::Message(bytes) if bytes.windows(5).any(|part| part == b"caf\xc3\xa9")
    ));
}

#[test]
fn pop3_lang_lists_selects_and_rejects_unsupported_ranges() {
    let mut session = Session::default();
    let capabilities = session.apply(parse("CAPA\r\n"));
    assert!(
        matches!(capabilities, Reply::Multiline(bytes) if bytes.windows(6).any(|part| part == b"LANG\r\n"))
    );
    assert!(matches!(
        session.apply(parse("LANG\r\n")),
        Reply::Multiline(bytes) if bytes == b"+OK Language listing follows\r\nen English\r\n.\r\n"
    ));
    assert!(
        matches!(session.apply(parse("LANG EN\r\n")), Reply::Ok(text) if text.starts_with("en "))
    );
    assert!(
        matches!(session.apply(parse("LANG *\r\n")), Reply::Ok(text) if text.starts_with("en "))
    );
    assert!(matches!(
        session.apply(parse("LANG de\r\n")),
        Reply::Error(_)
    ));
}

#[test]
fn maildrop_classifies_ascii_utf8_opaque_and_mixed_message_bytes() {
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

#[test]
fn ascii_pop3_session_never_sends_opaque_high_bit_octets() {
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

#[path = "listing_tests.rs"]
mod listing_tests;

#[path = "delete_tests.rs"]
mod delete_tests;

#[path = "mime_policy_tests.rs"]
mod mime_policy_tests;

#[test]
fn rset_and_empty_quit_require_an_explicit_update_completion() {
    let mut session = Session::default();
    let _ = session.apply(parse("USER pop\r\n"));
    let _ = session.authenticated(vec![message(3, b"Subject: retained\r\n\r\nbody")]);
    assert!(matches!(session.apply(parse("DELE 1\r\n")), Reply::Ok(_)));
    assert!(matches!(session.apply(parse("RSET\r\n")), Reply::Ok(_)));
    assert!(
        matches!(session.apply(parse("QUIT\r\n")), Reply::CommitDeletes(values) if values.is_empty())
    );
    assert!(matches!(session.commit_deletes(true), Reply::Ok(_)));
}
