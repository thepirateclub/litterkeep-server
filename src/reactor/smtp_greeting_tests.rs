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

/// RFC 3461 capability рекламируется после EHLO.
#[test]
fn ehlo_advertises_dsn_extension() {
    let mut session = smtp_session();
    ehlo(&mut session).expect("EHLO reply");
    assert!(session.output.windows(7).any(|part| part == b"250-DSN"));
}

/// MAIL/RCPT DSN параметры проходят ESMTP policy и попадают в typed envelope.
#[test]
fn dsn_parameters_survive_accepted_esmtp_transaction() {
    let mut session = smtp_session();
    session.smtp_greeting = smtp::GreetingMode::Extended;
    let mail = smtp::parse("MAIL FROM:<alice@test> RET=HDRS ENVID=id+20x\r\n").expect("DSN MAIL");
    let recipient = smtp::parse("RCPT TO:<bob@test> NOTIFY=FAILURE ORCPT=rfc822;bob@test\r\n")
        .expect("DSN RCPT");
    assert!(requirement(&mut session, &mail).is_none());
    assert!(requirement(&mut session, &recipient).is_none());

    let smtp::Command::MailWithDsn(mail, mail_dsn) = mail else {
        panic!("MAIL DSN parameters must remain typed");
    };
    super::super::smtp_protocol::apply_mail(&mut session, mail, Some(mail_dsn))
        .expect("accept MAIL");
    let SmtpState::Command(transaction) = &mut session.smtp else {
        panic!("MAIL leaves the SMTP transaction open");
    };
    let smtp::Command::RcptWithDsn(recipient) = recipient else {
        panic!("RCPT DSN parameters must remain typed");
    };
    transaction
        .accept_typed_recipient(recipient)
        .expect("accept RCPT");
    let envelope = transaction.take_envelope();
    assert_eq!(envelope.dsn.mail.envelope_id, Some(b"id x".to_vec()));
    assert_eq!(
        envelope.dsn.mail.return_content,
        Some(smtp::ReturnContent::Headers)
    );
    let accepted = envelope.dsn.recipients.first().expect("recipient metadata");
    assert!(matches!(
        accepted.dsn.notify,
        Some(smtp::NotifyPolicy::Conditions(_))
    ));
    assert_eq!(
        accepted
            .dsn
            .original_recipient
            .as_ref()
            .map(|value| value.address.as_slice()),
        Some(b"bob@test".as_slice())
    );
}

/// HELO без ESMTP остаётся без RFC 3461 параметров.
#[test]
fn dsn_parameters_still_require_ehlo() {
    let mut session = smtp_session();
    session.smtp_greeting = smtp::GreetingMode::Basic;
    let command = smtp::parse("MAIL FROM:<alice@test> ENVID=queue-9\r\n").expect("DSN MAIL");

    assert!(requirement(&mut session, &command).is_some());
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
