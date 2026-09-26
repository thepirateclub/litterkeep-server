//! Проверки ENABLE UTF8=ACCEPT на владельце настоящей IMAP-сессии.

use super::super::super::*;
use super::super::imap_search_error::reject_utf8_charset;
use super::execute;

#[test]
fn authenticated_enable_changes_parser_mode_and_reports_enabled_extension() {
    let (mut session, _peer) = session(true);
    execute(
        &mut session,
        "A1",
        vec![imap::enable::Capability::Utf8Accept],
    )
    .expect("ENABLE");
    assert_eq!(
        session.imap_string_mode,
        imap::tokenizer::StringMode::Utf8Accept
    );
    assert_eq!(
        session.output,
        b"* ENABLED UTF8=ACCEPT\r\nA1 OK ENABLE completed\r\n"
    );
}

#[test]
fn unauthenticated_enable_does_not_change_parser_mode() {
    let (mut session, _peer) = session(false);
    execute(
        &mut session,
        "A2",
        vec![imap::enable::Capability::Utf8Accept],
    )
    .expect("ENABLE rejection");
    assert_eq!(
        session.imap_string_mode,
        imap::tokenizer::StringMode::Legacy
    );
    assert_eq!(session.output, b"A2 NO authenticate first\r\n");
}

#[test]
fn utf8_session_rejects_explicit_search_charset_before_store_routing() {
    let (mut session, _peer) = session(true);
    session.imap_string_mode = imap::tokenizer::StringMode::Utf8Accept;
    session.selected_mailbox = Some("INBOX".into());
    session.saved_search_uids.insert(4);
    let parsed = imap::parse("A3 SEARCH RETURN (SAVE) CHARSET utf-8 ALL\r\n");
    let rejected =
        reject_utf8_charset(&mut session, &parsed.tag, &parsed.command).expect("SEARCH policy");
    assert!(rejected);
    assert_eq!(
        session.output,
        b"A3 BAD CHARSET is not allowed after ENABLE UTF8=ACCEPT\r\n"
    );
    assert!(session.saved_search_uids.is_empty());
}

fn session(authenticated: bool) -> (Session, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let peer = TcpStream::connect(listener.local_addr().expect("address")).expect("peer");
    let (stream, _) = listener.accept().expect("accept");
    let mut session = Session::new(Kind::Imap, stream, 1, None, None).expect("session");
    session.output.clear();
    if authenticated {
        session.authenticated_account = Some("alice".into());
    }
    (session, peer)
}
