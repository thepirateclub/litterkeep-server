//! Parser, validator и executor семантика RFC 5230 vacation action.

use super::{Action, VacationAction, execute, parse};

#[test]
fn vacation_requires_capability_and_parses_typed_parameters() {
    assert!(parse("vacation \"Away\";").is_err());
    let script = parse(
        "require \"vacation\"; vacation :days 14 :subject \"Gone\" :from \"away@test\" :addresses [\"bob@test\", \"bobby@test\"] :mime :handle \"summer\" \"Content-Type: text/plain\r\n\r\nAway\r\n\";",
    )
    .expect("valid vacation script");
    let Some(Action::Vacation(action)) = script.commands.first().and_then(action_of) else {
        panic!("typed vacation action expected");
    };
    assert_eq!(
        action,
        &VacationAction {
            days: Some(14),
            subject: Some("Gone".into()),
            from: Some("away@test".into()),
            addresses: vec!["bob@test".into(), "bobby@test".into()],
            mime: true,
            handle: Some("summer".into()),
            reason: "Content-Type: text/plain\r\n\r\nAway\r\n".into(),
        }
    );
}

#[test]
fn vacation_is_single_per_script_including_nested_branches() {
    let duplicate =
        "require \"vacation\"; if true { vacation \"first\"; } else { vacation \"second\"; }";
    assert!(parse(duplicate).is_err());
}

#[test]
fn vacation_preserves_keep_and_coexists_with_discard() {
    let vacation_only = parse("require \"vacation\"; vacation \"Away\";").expect("valid vacation");
    assert!(execute(&vacation_only, &[], 1).expect("keep").keep);

    let script =
        parse("require \"vacation\"; vacation \"Away\"; discard;").expect("valid mixed actions");
    let plan = execute(&script, &[], 1).expect("plan");
    assert!(!plan.keep);
    assert!(plan.discard);
    assert_eq!(plan.vacations.len(), 1);
    assert!(plan.redirects.is_empty());
}

#[test]
fn vacation_rejects_duplicate_parameters_and_invalid_addresses() {
    assert!(parse("require \"vacation\"; vacation :days 1 :days 2 \"Away\";").is_err());
    assert!(parse("require \"vacation\"; vacation :from \"bad address\" \"Away\";").is_err());
    assert!(
        parse("require \"vacation\"; vacation :addresses [\"one@test, two@test\"] \"Away\";")
            .is_err()
    );
}

#[test]
fn response_identity_uses_handle_or_distinct_typed_parameters() {
    let first = parse("require \"vacation\"; vacation :subject \"A\" \"Away\";").expect("first");
    let second = parse("require \"vacation\"; vacation :subject \"B\" \"Away\";").expect("second");
    let first_action = vacation_action(&first);
    let second_action = vacation_action(&second);
    assert_ne!(
        super::response_identity(first_action).expect("identity"),
        super::response_identity(second_action).expect("identity"),
    );
}

#[test]
fn generated_utf8_reply_has_safe_headers_and_decodable_body() {
    let script =
        parse("require \"vacation\"; vacation :subject \"Отсутствую\" \"Вернусь завтра\";")
            .expect("script");
    let headers = vec![
        ("subject".to_owned(), "Original".to_owned()),
        ("message-id".to_owned(), "<one@peer.test>".to_owned()),
    ];
    let response = super::build_vacation_response(
        vacation_action(&script),
        "bob@litterkeep.test",
        "alice@peer.test",
        &headers,
        1_790_000_000,
    )
    .expect("reply");
    assert!(
        response
            .windows(b"Auto-Submitted: auto-replied\r\n".len())
            .any(|part| part == b"Auto-Submitted: auto-replied\r\n")
    );
    assert!(
        response
            .windows(b"In-Reply-To: <one@peer.test>\r\n".len())
            .any(|part| part == b"In-Reply-To: <one@peer.test>\r\n")
    );
    let parsed = crate::mime::header_block::parse(&response).expect("response headers");
    let body = response.get(parsed.body_offset..).expect("body");
    assert_eq!(
        crate::mime::quoted_printable::decode(body).expect("decode"),
        "Вернусь завтра\r\n".as_bytes()
    );
}

#[test]
fn mime_vacation_accepts_ascii_entity_and_rejects_unsafe_entities() {
    let valid = mime_script(
        "Content-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\nOl=E1\r\n",
    );
    assert!(parse(&valid).is_ok());
    assert!(
        parse(&mime_script(
            "Bcc: victim@test\r\nContent-Type: text/plain\r\n\r\nhello\r\n"
        ))
        .is_err()
    );
    assert!(
        parse(&mime_script(
            "Content-Type: text/plain\r\nX-Name: café\r\n\r\nhello\r\n"
        ))
        .is_err()
    );
    assert!(parse(&mime_script("Content-Type: text/plain\r\n\r\nпривет\r\n")).is_err());
}

#[test]
fn mime_vacation_rejects_malformed_transfer_encoding_and_duplicate_fields() {
    for entity in [
        "Content-Type: text/plain\r\nContent-Transfer-Encoding: base64\r\n\r\n%%%\r\n",
        "Content-Type: text/plain\r\nContent-Transfer-Encoding: base64\r\n\r\naGVsbG8=AAAA\r\n",
        "Content-Type: text/plain\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\nbad=QZ\r\n",
        "Content-Type: text/plain\r\nContent-Transfer-Encoding: 7bit\r\n\r\nhello\u{00e9}\r\n",
        "Content-Type: text/plain\r\nContent-Transfer-Encoding: base64\r\nContent-Transfer-Encoding: 7bit\r\n\r\naGVsbG8=\r\n",
    ] {
        assert!(parse(&mime_script(entity)).is_err(), "accepted {entity:?}");
    }
}

fn mime_script(entity: &str) -> String {
    format!("require \"vacation\"; vacation :mime \"{entity}\";")
}

#[test]
fn response_policy_suppresses_null_daemon_list_and_automated_messages() {
    let ordinary = vec![("auto-submitted".to_owned(), "no".to_owned())];
    assert!(super::may_respond("alice@peer.test", &ordinary));
    assert!(!super::may_respond("", &[]));
    assert!(!super::may_respond("mailer-daemon@peer.test", &[]));
    assert!(!super::may_respond(
        "alice@peer.test",
        &[("list-id".to_owned(), "<announce.test>".to_owned())],
    ));
    assert!(!super::may_respond(
        "alice@peer.test",
        &[("auto-submitted".to_owned(), "auto-generated".to_owned())],
    ));
}

#[test]
fn vacation_requires_a_personal_header_and_addresses_are_additional() {
    let extra = vec!["alternate@litterkeep.test".to_owned()];
    let final_recipient = "bob@litterkeep.test";
    assert!(super::personal_message(
        final_recipient,
        &extra,
        &[("to".into(), "bob@litterkeep.test".into())],
    ));
    assert!(super::personal_message(
        final_recipient,
        &extra,
        &[("cc".into(), "alternate@litterkeep.test".into())],
    ));
    assert!(!super::personal_message(
        final_recipient,
        &extra,
        &[("x-recipient".into(), "bob@litterkeep.test".into())],
    ));
    assert!(!super::personal_message(
        final_recipient,
        &extra,
        &[("to".into(), "alice@litterkeep.test".into())],
    ));
    assert!(!super::personal_message(final_recipient, &extra, &[]));
}

fn action_of(command: &super::ast::Command) -> Option<&Action> {
    match command {
        super::ast::Command::Action(action) => Some(action),
        super::ast::Command::If { .. } => None,
    }
}

fn vacation_action(script: &super::Script) -> &VacationAction {
    let Some(Action::Vacation(action)) = script.commands.first().and_then(action_of) else {
        panic!("vacation action expected");
    };
    action
}
