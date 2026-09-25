//! Проверки command grammar и security states без socket implementation.

use super::*;

#[test]
fn tls_precedes_plain_auth_and_authenticated_commands() {
    let mut session = Session::default();
    assert!(matches!(
        session.apply(parse("AUTHENTICATE \"PLAIN\" \"AGJvYgBw\""), false),
        Reply::Text(_)
    ));
    assert!(matches!(
        session.apply(parse("AUTHENTICATE \"PLAIN\" \"AGJvYgBw\""), true),
        Reply::Authenticate { .. }
    ));
    assert!(matches!(session.authenticated(true), Reply::Text(_)));
    assert!(matches!(
        session.apply(parse("PUTSCRIPT \"invoices\" \"keep;\""), true),
        Reply::Put { .. }
    ));
}

#[test]
fn command_parser_retains_quoted_script_and_rejects_extra_argument() {
    assert_eq!(
        parse("CHECKSCRIPT \"require [\\\"fileinto\\\"]; keep;\""),
        Command::CheckScript("require [\"fileinto\"]; keep;".into())
    );
    assert_eq!(parse("LISTSCRIPTS extra"), Command::Invalid);
}

#[test]
fn capability_advertises_supported_sieve_extensions() {
    let mut session = Session::default();
    let Reply::Text(response) = session.apply(parse("CAPABILITY"), false) else {
        panic!("CAPABILITY должен вернуть текстовый ответ");
    };
    assert!(
        response
            .windows(b"\"SIEVE\" \"fileinto subaddress envelope copy\"".len())
            .any(|part| part == b"\"SIEVE\" \"fileinto subaddress envelope copy\"")
    );
}
