//! Проверки рекламирования SMTP extensions и отказа неподдержанного BINARYMIME.
use super::*;

/// EHLO не рекламирует BINARYMIME до завершения всего RFC 3030 профиля.
#[test]
fn ehlo_keeps_binarymime_capability_hidden() {
    let mut session = smtp_session();
    ehlo(&mut session).expect("EHLO reply");
    assert!(!session.output.windows(10).any(|part| part == b"BINARYMIME"));
}

/// Допустимый RFC 3030 параметр отклоняется как неподдержанный без envelope.
#[test]
fn binarymime_mail_is_rejected_without_opening_transaction() {
    let mut session = smtp_session();
    session.smtp_greeting = smtp::GreetingMode::Extended;
    let command =
        smtp::parse("MAIL FROM:<alice@test> BODY=BINARYMIME\r\n").expect("RFC 3030 MAIL syntax");

    requirement(&mut session, &command)
        .expect("unsupported BINARYMIME response")
        .expect("write SMTP reply");

    assert_eq!(
        session.output,
        b"220 litterkeep.test ESMTP litterkeep-server\r\n555 5.5.4 BINARYMIME is not supported\r\n"
    );
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
