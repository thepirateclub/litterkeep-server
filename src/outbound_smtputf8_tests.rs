//! Проверяет выбор SMTPUTF8 параметров исходящим транспортом.
use crate::outbound::{self, MailCommand};
use std::io::Cursor;

/// Для UTF-8 заголовков peer должен объявить SMTPUTF8 вместе с 8BITMIME.
#[test]
fn utf8_header_requires_both_peer_capabilities_and_sends_both_parameters() {
    let peer = outbound::read_ehlo(&mut Cursor::new(
        b"250-peer.test\r\n250-8BITMIME\r\n250 SMTPUTF8\r\n",
    ))
    .expect("capabilities");
    assert!(matches!(
        outbound::build_mail_command(
            "alice@test",
            false,
            "Subject: Привет\r\n\r\nbody".as_bytes(),
            &peer,
        ),
        MailCommand::Ready(command)
            if command == "MAIL FROM:<alice@test> BODY=8BITMIME SMTPUTF8\r\n"
    ));

    let missing_utf8 = outbound::read_ehlo(&mut Cursor::new(b"250-peer.test\r\n250 8BITMIME\r\n"))
        .expect("8BITMIME-only capabilities");
    assert!(matches!(
        outbound::build_mail_command(
            "alice@test",
            false,
            "Subject: Привет\r\n\r\nbody".as_bytes(),
            &missing_utf8,
        ),
        MailCommand::PermanentFailure
    ));
}

/// Международный envelope требует MAIL SMTPUTF8 даже при ASCII message headers.
#[test]
fn utf8_envelope_requires_smtputf8_and_8bitmime_capabilities() {
    let peer = outbound::read_ehlo(&mut Cursor::new(
        b"250-peer.test\r\n250-8BITMIME\r\n250 SMTPUTF8\r\n",
    ))
    .expect("capabilities");
    assert!(matches!(
        outbound::build_mail_command("алиса@test", true, b"Subject: plain\r\n\r\nbody", &peer),
        MailCommand::Ready(command)
            if command == "MAIL FROM:<алиса@test> SMTPUTF8\r\n"
    ));

    let missing_utf8 = outbound::read_ehlo(&mut Cursor::new(b"250-peer.test\r\n250 8BITMIME\r\n"))
        .expect("8BITMIME-only capabilities");
    assert!(matches!(
        outbound::build_mail_command(
            "alice@test",
            true,
            b"Subject: plain\r\n\r\nbody",
            &missing_utf8
        ),
        MailCommand::PermanentFailure
    ));
}
