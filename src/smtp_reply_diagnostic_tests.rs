//! Проверки сериализации diagnostic в совместимый с queue-журналом токен.

use super::{RemoteMtaIdentity, SmtpReplyDiagnostic};
use std::net::IpAddr;

#[test]
fn journal_round_trip_preserves_multiline_peer_text() {
    let diagnostic =
        SmtpReplyDiagnostic::new(550, "5.1.1 mailbox unavailable additional context".into())
            .expect("typed diagnostic")
            .with_remote_mta(RemoteMtaIdentity::from_ip(
                "2001:db8::25".parse::<IpAddr>().expect("IPv6 address"),
            ));
    let recovered = SmtpReplyDiagnostic::from_journal_token(&diagnostic.journal_token())
        .expect("recover diagnostic");
    assert_eq!(recovered, diagnostic);
}

#[test]
fn remote_dns_identity_rejects_invalid_dns_labels() {
    assert!(RemoteMtaIdentity::from_journal_token("D2D2D").is_err());
    assert!(RemoteMtaIdentity::from_journal_token("D2D2D2D2D2D").is_err());
}

#[test]
fn journal_parser_rejects_bad_class_hex_and_control_text() {
    assert!(SmtpReplyDiagnostic::from_journal_token("250:41").is_err());
    assert!(SmtpReplyDiagnostic::from_journal_token("550:4G").is_err());
    assert!(SmtpReplyDiagnostic::from_journal_token("550:0A").is_err());
}
