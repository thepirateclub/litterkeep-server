//! Регрессии typed-команды и TLS-политики SCRAM-SHA-256.

use super::{Command, Reply, Session, parse};

/// Проверяет механизм, TLS gate, initial response и отменяемый challenge.
#[test]
fn command_is_typed_tls_only_and_challenge_is_cancelable() {
    let client_first = crate::sasl::encode_base64(b"n,,n=bob,r=client-nonce");
    assert_eq!(
        parse(&format!(
            "AUTHENTICATE \"sCrAm-ShA-256\" \"{client_first}\""
        )),
        Command::AuthenticateScramSha256(Some(client_first.clone()))
    );
    let mut session = Session::default();
    assert!(matches!(
        session.apply(
            Command::AuthenticateScramSha256(Some(client_first.clone())),
            false,
            true,
        ),
        Reply::Text(response) if response == b"NO \"encryption required\"\r\n"
    ));
    assert!(matches!(
        session.apply(
            Command::AuthenticateScramSha256(Some(client_first.clone())),
            true,
            false,
        ),
        Reply::ScramSha256ClientFirst { response, channel_binding: None } if response == client_first
    ));
    let mut challenge_session = Session::default();
    assert!(matches!(
        challenge_session.apply(parse("AUTHENTICATE \"SCRAM-SHA-256\""), true, false),
        Reply::AuthenticateChallenge
    ));
    assert!(challenge_session.awaiting_authentication_response());
    assert!(matches!(
        challenge_session.apply(Command::AuthenticateResponse("*".into()), true, false),
        Reply::Text(response) if response == b"NO \"authentication cancelled\"\r\n"
    ));
    assert!(!challenge_session.awaiting_authentication_response());
}

/// PLUS остаётся скрытым без exporter и входит в SASL list при TLS binding.
#[test]
fn plus_capability_requires_a_tls_exporter() {
    let plain_tls = crate::managesieve::greeting(true, false);
    assert!(
        !plain_tls
            .windows(b"SCRAM-SHA-256-PLUS".len())
            .any(|part| part == b"SCRAM-SHA-256-PLUS")
    );
    let bound_tls = crate::managesieve::greeting_with_channel_binding(true, false, true);
    assert!(
        bound_tls
            .windows(b"SCRAM-SHA-256-PLUS".len())
            .any(|part| part == b"SCRAM-SHA-256-PLUS")
    );
    let mut plaintext = Session::default();
    assert!(matches!(
        plaintext.apply_with_channel_binding(
            parse("AUTHENTICATE \"SCRAM-SHA-256-PLUS\""),
            false,
            true,
            Some(b"unexpected exporter")
        ),
        Reply::Text(response) if response == b"NO \"channel binding unavailable\"\r\n"
    ));
}
