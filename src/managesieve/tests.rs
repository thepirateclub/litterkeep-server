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
            .windows(b"\"SASL\" \"PLAIN\"".len())
            .any(|part| part == b"\"SASL\" \"PLAIN\"")
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
            .windows(b"\"SASL\" \"PLAIN\"".len())
            .any(|part| part == b"\"SASL\" \"PLAIN\"")
    );
    assert!(
        !implicit_tls
            .windows(b"\"STARTTLS\"".len())
            .any(|part| part == b"\"STARTTLS\"")
    );
}
