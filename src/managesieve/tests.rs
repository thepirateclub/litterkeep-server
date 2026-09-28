//! Проверки command grammar и security states без socket implementation.

use super::*;

#[test]
fn tls_precedes_plain_auth_and_authenticated_commands() {
    let mut session = Session::default();
    assert!(matches!(
        session.apply(parse("AUTHENTICATE \"PLAIN\" \"AGJvYgBw\""), false, true),
        Reply::Text(_)
    ));
    assert!(matches!(
        session.authenticated("bob".into(), true, false),
        Reply::Text(response) if response == b"NO \"encryption required\"\r\n"
    ));
    assert!(matches!(
        session.apply(parse("AUTHENTICATE \"PLAIN\" \"AGJvYgBw\""), true, false),
        Reply::Authenticate { .. }
    ));
    let Reply::Text(authenticated) = session.authenticated("bob".into(), true, true) else {
        panic!("AUTHENTICATE должен вернуть новый capability block");
    };
    assert!(authenticated.ends_with(b"OK \"authenticated\"\r\n"));
    assert!(
        authenticated
            .windows(b"\"OWNER\" \"bob\"".len())
            .any(|part| part == b"\"OWNER\" \"bob\"")
    );
    assert!(
        !authenticated
            .windows(b"\"STARTTLS\"".len())
            .any(|part| part == b"\"STARTTLS\"")
    );
    assert!(matches!(
        session.apply(parse("PUTSCRIPT \"invoices\" \"keep;\""), true, false),
        Reply::Put { .. }
    ));
    assert!(matches!(
        session.apply(parse("STARTTLS"), true, false),
        Reply::Text(response) if response == b"NO \"TLS is unavailable or already active\"\r\n"
    ));
}

#[test]
fn plain_authentication_without_initial_response_uses_a_challenge() {
    let mut session = Session::default();
    assert_eq!(
        parse("AUTHENTICATE \"PLAIN\""),
        Command::AuthenticatePlain(None)
    );
    assert!(matches!(
        session.apply(parse("AUTHENTICATE \"PLAIN\""), true, false),
        Reply::AuthenticateChallenge
    ));
    assert!(session.awaiting_plain_response());
    assert!(matches!(
        session.apply(
            Command::AuthenticateResponse("AGJvYgBw".into()),
            true,
            false
        ),
        Reply::Authenticate { response } if response == "AGJvYgBw"
    ));
    assert!(!session.awaiting_plain_response());
}

#[test]
fn plain_authentication_challenge_can_be_cancelled_and_retried() {
    let mut session = Session::default();
    let _ = session.apply(parse("AUTHENTICATE \"PLAIN\""), true, false);
    assert!(matches!(
        session.apply(
            Command::AuthenticateResponse("*".into()),
            true,
            false
        ),
        Reply::Text(response) if response == b"NO \"authentication cancelled\"\r\n"
    ));
    assert!(!session.awaiting_plain_response());
    assert!(matches!(
        session.apply(parse("AUTHENTICATE \"PLAIN\" \"AGJvYgBw\""), true, false),
        Reply::Authenticate { .. }
    ));
}

#[test]
fn scram_sha1_command_is_typed_and_available_only_after_tls() {
    let client_first = crate::sasl::encode_base64(b"n,,n=bob,r=client-nonce");
    let command = parse(&format!("AUTHENTICATE \"sCrAm-ShA-1\" \"{client_first}\""));
    assert_eq!(
        command,
        Command::AuthenticateScramSha1(Some(client_first.clone()))
    );
    assert_eq!(
        parse("AUTHENTICATE \"SCRAM-SHA-1\""),
        Command::AuthenticateScramSha1(None)
    );
    assert_eq!(
        parse("AUTHENTICATE \"CRAM-MD5\" \"anything\""),
        Command::Invalid
    );
    let mut session = Session::default();
    assert!(matches!(
        session.apply(
            Command::AuthenticateScramSha1(Some(client_first.clone())),
            false,
            true,
        ),
        Reply::Text(response) if response == b"NO \"encryption required\"\r\n"
    ));
    assert!(matches!(
        session.apply(
            Command::AuthenticateScramSha1(Some(client_first.clone())),
            true,
            false,
        ),
        Reply::ScramClientFirst { response } if response == client_first
    ));
}

#[test]
fn scram_without_initial_response_uses_cancelable_challenge_state() {
    let mut session = Session::default();
    assert!(matches!(
        session.apply(parse("AUTHENTICATE \"SCRAM-SHA-1\""), true, false),
        Reply::AuthenticateChallenge
    ));
    assert!(session.awaiting_authentication_response());
    assert!(matches!(
        session.apply(Command::AuthenticateResponse("*".into()), true, false),
        Reply::Text(response) if response == b"NO \"authentication cancelled\"\r\n"
    ));
    assert!(!session.awaiting_authentication_response());
}

#[test]
fn authentication_response_parser_accepts_one_string_only() {
    assert_eq!(
        parse_authentication_response("\"AGJvYgBw\""),
        Some("AGJvYgBw".into())
    );
    assert_eq!(parse_authentication_response("\"*\""), Some("*".into()));
    assert_eq!(parse_authentication_response("\"unterminated"), None);
    assert_eq!(parse_authentication_response("\"reply\" extra"), None);
    assert_eq!(parse_authentication_response("reply"), None);
}

#[test]
fn command_parser_retains_quoted_script_and_rejects_extra_argument() {
    assert_eq!(
        parse("CHECKSCRIPT \"require [\\\"fileinto\\\"]; keep;\""),
        Command::CheckScript("require [\"fileinto\"]; keep;".into())
    );
    assert_eq!(parse("LISTSCRIPTS extra"), Command::Invalid);
    assert_eq!(parse("NOOP"), Command::Noop(None));
    assert_eq!(
        parse("NOOP \"sync-42\""),
        Command::Noop(Some("sync-42".into()))
    );
    assert_eq!(
        parse("NOOP \"a \\\"b\\\\c\""),
        Command::Noop(Some("a \"b\\c".into()))
    );
    assert_eq!(parse("NOOP \"tag\" extra"), Command::Invalid);
    assert_eq!(parse("SETACTIVE"), Command::Invalid);
    assert_eq!(parse("SETACTIVE \"\""), Command::SetActive(None));
    assert_eq!(
        parse("SETACTIVE \"invoices\""),
        Command::SetActive(Some("invoices".into()))
    );
    assert_eq!(parse("SETACTIVE \"\" extra"), Command::Invalid);
    assert_eq!(parse("UNAUTHENTICATE"), Command::Unauthenticate);
    assert_eq!(parse("UNAUTHENTICATE extra"), Command::Invalid);
}

#[test]
fn command_parser_rejects_unterminated_or_invalid_quoted_strings() {
    assert_eq!(parse("CAPABILITY \"unterminated"), Command::Invalid);
    assert_eq!(parse("NOOP \"bad\\q\""), Command::Invalid);
    assert_eq!(parse("NOOP \"dangling\\"), Command::Invalid);
}

#[test]
fn noop_is_allowed_before_authentication_and_echoes_quoted_tag() {
    let mut session = Session::default();
    assert!(matches!(
        session.apply(parse("NOOP"), false, true),
        Reply::Text(response) if response == b"OK \"noop\"\r\n"
    ));
    assert!(matches!(
        session.apply(parse("NOOP \"sync-42\""), false, true),
        Reply::Text(response) if response == b"OK (TAG \"sync-42\") \"noop\"\r\n"
    ));
    assert!(matches!(
        session.apply(parse("NOOP \"a \\\"b\\\\c\""), false, true),
        Reply::Text(response) if response == b"OK (TAG \"a \\\"b\\\\c\") \"noop\"\r\n"
    ));
}

#[test]
fn capability_advertises_supported_sieve_extensions() {
    let mut session = Session::default();
    let Reply::Text(response) = session.apply(parse("CAPABILITY"), false, true) else {
        panic!("CAPABILITY должен вернуть текстовый ответ");
    };
    assert!(
        response
            .windows(
                b"\"SIEVE\" \"fileinto subaddress envelope copy vacation vacation-seconds\"".len()
            )
            .any(|part| part
                == b"\"SIEVE\" \"fileinto subaddress envelope copy vacation vacation-seconds\"")
    );
    assert!(
        response
            .windows(b"\"VERSION\" \"1.0\"".len())
            .any(|part| part == b"\"VERSION\" \"1.0\"")
    );
    assert!(
        response
            .windows(b"\"UNAUTHENTICATE\"".len())
            .any(|part| part == b"\"UNAUTHENTICATE\"")
    );
    assert!(
        response
            .windows(b"\"SASL\" \"\"".len())
            .any(|part| part == b"\"SASL\" \"\"")
    );
    assert!(
        response
            .windows(b"\"STARTTLS\"".len())
            .any(|part| part == b"\"STARTTLS\"")
    );

    let Reply::Text(encrypted) = session.apply(parse("CAPABILITY"), true, false) else {
        panic!("TLS CAPABILITY должен вернуть текстовый ответ");
    };
    assert!(
        encrypted
            .windows(b"\"SASL\" \"PLAIN SCRAM-SHA-1 SCRAM-SHA-256\"".len())
            .any(|part| part == b"\"SASL\" \"PLAIN SCRAM-SHA-1 SCRAM-SHA-256\"")
    );
    assert!(
        !encrypted
            .windows(b"\"STARTTLS\"".len())
            .any(|part| part == b"\"STARTTLS\"")
    );
    assert!(matches!(
        session.apply(parse("STARTTLS"), true, false),
        Reply::Text(response) if response == b"NO \"TLS is unavailable or already active\"\r\n"
    ));

    let implicit_tls = greeting(true, false);
    assert!(
        implicit_tls
            .windows(b"\"SASL\" \"PLAIN SCRAM-SHA-1 SCRAM-SHA-256\"".len())
            .any(|part| part == b"\"SASL\" \"PLAIN SCRAM-SHA-1 SCRAM-SHA-256\"")
    );
    assert!(
        !implicit_tls
            .windows(b"\"STARTTLS\"".len())
            .any(|part| part == b"\"STARTTLS\"")
    );
    assert!(
        implicit_tls
            .windows(b"\"VERSION\" \"1.0\"".len())
            .any(|part| part == b"\"VERSION\" \"1.0\"")
    );
}

#[test]
fn unauthenticate_clears_owner_keeps_tls_and_allows_authentication_again() {
    let mut session = Session::default();
    let _ = session.authenticated("bob".into(), true, true);
    let Reply::Unauthenticated(capabilities) = session.apply(parse("UNAUTHENTICATE"), true, false)
    else {
        panic!("UNAUTHENTICATE должен переиздать capabilities");
    };
    assert!(capabilities.ends_with(b"OK \"unauthenticated\"\r\n"));
    assert!(
        !capabilities
            .windows(b"\"OWNER\"".len())
            .any(|part| part == b"\"OWNER\"")
    );
    assert!(
        capabilities
            .windows(b"\"SASL\" \"PLAIN SCRAM-SHA-1 SCRAM-SHA-256\"".len())
            .any(|part| part == b"\"SASL\" \"PLAIN SCRAM-SHA-1 SCRAM-SHA-256\"")
    );
    assert!(matches!(
        session.apply(parse("LISTSCRIPTS"), true, false),
        Reply::Text(response) if response == b"NO \"invalid command for state\"\r\n"
    ));
    assert!(matches!(
        session.apply(parse("AUTHENTICATE \"PLAIN\" \"AGJvYgBw\""), true, false),
        Reply::Authenticate { .. }
    ));
}
