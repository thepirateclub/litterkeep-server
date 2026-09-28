//! Проверки рекламирования SMTP extensions и принятия их MAIL параметров.
use super::*;

/// BINARYMIME рекламируется вместе с уже реализованным CHUNKING.
#[test]
fn ehlo_advertises_binarymime_with_chunking() {
    let mut session = smtp_session();
    ehlo(&mut session).expect("EHLO reply");
    assert!(session.output.windows(10).any(|part| part == b"BINARYMIME"));
    assert!(session.output.windows(8).any(|part| part == b"CHUNKING"));
}

/// SMTPUTF8 рекламируется только вместе с обязательной поддержкой 8BITMIME.
#[test]
fn ehlo_advertises_smtputf8_and_8bitmime() {
    let mut session = smtp_session();
    ehlo(&mut session).expect("EHLO reply");
    assert!(session.output.windows(8).any(|part| part == b"SMTPUTF8"));
    assert!(session.output.windows(8).any(|part| part == b"8BITMIME"));
}

/// DSN MAIL и RCPT grammar принимаются после ESMTP greeting.
#[test]
fn dsn_commands_pass_extended_greeting_gate() {
    let mut session = smtp_session();
    session.smtp_greeting = smtp::GreetingMode::Extended;
    let mail = smtp::parse("MAIL FROM:<alice@test> ENVID=id+20x\r\n").expect("DSN MAIL");
    let recipient = smtp::parse("RCPT TO:<bob@test> NOTIFY=FAILURE ORCPT=rfc822;bob@test\r\n")
        .expect("DSN RCPT");

    assert!(requirement(&mut session, &mail).is_some());
    assert!(requirement(&mut session, &recipient).is_some());
    assert!(session.output.windows(3).any(|part| part == b"555"));
}

/// Поддержанный RFC 3030 параметр проходит greeting gate без мутации envelope.
#[test]
fn binarymime_mail_is_accepted_after_ehlo() {
    let mut session = smtp_session();
    session.smtp_greeting = smtp::GreetingMode::Extended;
    let command =
        smtp::parse("MAIL FROM:<alice@test> BODY=BINARYMIME\r\n").expect("RFC 3030 MAIL syntax");

    assert!(requirement(&mut session, &command).is_none());
    assert!(matches!(session.smtp, SmtpState::Command(_)));
}

/// Создаёт реальный неблокирующий socket для владельца SMTP session.
fn smtp_session() -> Session {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test socket");
    let client = std::net::TcpStream::connect(listener.local_addr().expect("local address"))
        .expect("connect test socket");
    let (server, _) = listener.accept().expect("accept test socket");
    drop(client);
    Session::new(Kind::Smtp, server, 1, None, None).expect("create SMTP session")
}
