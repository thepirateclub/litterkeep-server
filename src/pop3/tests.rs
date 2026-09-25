//! Проверки POP3 FSM, snapshot и отложенного удаления.

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
        session.apply(parse("PASS password\r\n")),
        Reply::Authenticate { .. }
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
        session.apply(parse("AUTH PLAIN\r\n")),
        Reply::SaslContinue
    ));
    assert!(session.awaiting_sasl_response());
    assert!(matches!(
        session.sasl_plain_response(&response),
        Reply::Authenticate { username, password }
            if username == "bob" && password == b"secret"
    ));

    assert!(matches!(
        session.apply(parse("AUTH PLAIN\r\n")),
        Reply::SaslContinue
    ));
    assert!(matches!(session.sasl_plain_response("*"), Reply::Error(_)));
    assert!(!session.awaiting_sasl_response());
}

#[test]
fn retrieve_uidl_reset_and_quit_keep_delete_semantics_explicit() {
    let mut session = Session::default();
    let _ = session.apply(parse("USER pop\r\n"));
    let _ = session.authenticated(vec![message(9, b"Subject: one\r\n\r\n.first\r\nbody")]);
    assert!(
        matches!(session.apply(parse("UIDL 1\r\n")), Reply::Multiline(value) if value.windows(4).any(|part| part == b"0009"))
    );
    assert!(
        matches!(session.apply(parse("RETR 1\r\n")), Reply::Message(value) if value.windows(8).any(|part| part == b"..first\r"))
    );
    assert!(matches!(session.apply(parse("DELE 1\r\n")), Reply::Ok(_)));
    assert!(matches!(session.apply(parse("RSET\r\n")), Reply::Ok(_)));
    assert!(matches!(session.apply(parse("DELE 1\r\n")), Reply::Ok(_)));
    assert!(
        matches!(session.apply(parse("QUIT\r\n")), Reply::CommitDeletes(values) if values == vec![9])
    );
    assert!(matches!(session.commit_deletes(true), Reply::Ok(_)));
}

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
