//! Проверяет POP3 DELE: повторный номер запрещён до сброса через RSET.

use super::*;
use crate::{mail_date::MailDate, mail_flags::MessageFlags, store::Message};
use std::path::PathBuf;

/// Удаление обратимо через RSET, но повторный DELE до него получает -ERR.
#[test]
fn repeated_delete_is_rejected_and_reset_allows_a_new_marker() {
    let mut session = Session::default();
    let _ = session.apply(parse("USER pop\r\n"));
    let message = Message {
        uid: 9,
        path: PathBuf::new(),
        bytes: b"Subject: one\r\n\r\nbody".to_vec(),
        flags: MessageFlags::default(),
        keywords: std::collections::BTreeSet::new(),
        internal_date: MailDate::from_unix_seconds(0),
    };
    let _ = session.authenticated(vec![message]);
    assert!(matches!(session.apply(parse("DELE 1\r\n")), Reply::Ok(_)));
    assert!(matches!(
        session.apply(parse("DELE 1\r\n")),
        Reply::Error(_)
    ));
    assert!(matches!(session.apply(parse("RSET\r\n")), Reply::Ok(_)));
    assert!(matches!(session.apply(parse("DELE 1\r\n")), Reply::Ok(_)));
    assert!(matches!(
        session.apply(parse("QUIT\r\n")),
        Reply::CommitDeletes(values) if values == vec![9]
    ));
    assert!(matches!(session.commit_deletes(true), Reply::Ok(_)));
}
