//! Проверка RFC 3464 диагностического ответа SMTP в recipient block.

use super::{Failure, ReportOutcome, render_with_diagnostic};
use crate::{
    queue::DsnDeliveryMetadata,
    smtp_reply_diagnostic::{RemoteMtaIdentity, SmtpReplyDiagnostic},
};

#[test]
fn outbound_smtp_reply_is_serialized_as_diagnostic_code() {
    let metadata = DsnDeliveryMetadata {
        mail: Default::default(),
        recipient: None,
        origin_recipients: Vec::new(),
    };
    let diagnostic = SmtpReplyDiagnostic::new(550, "5.1.1 mailbox unavailable".into())
        .expect("peer diagnostic")
        .with_remote_mta(RemoteMtaIdentity::from_ip(
            "192.0.2.25".parse().expect("IPv4 address"),
        ));
    let report = render_with_diagnostic(
        "sender@outside.example",
        "target@peer.example",
        42,
        b"Subject: source\r\n\r\nbody",
        ReportOutcome::Failure(Failure::PermanentSmtp),
        Some(&metadata),
        Some(&diagnostic),
    );
    assert!(contains(
        &report,
        b"Diagnostic-Code: smtp; 550 5.1.1 mailbox unavailable\r\n"
    ));
    assert!(contains(&report, b"Remote-MTA: dns; [192.0.2.25]\r\n"));
}

#[test]
fn missing_peer_diagnostic_does_not_invent_remote_response_text() {
    let report = render_with_diagnostic(
        "sender@outside.example",
        "target@peer.example",
        43,
        b"Subject: source\r\n\r\nbody",
        ReportOutcome::Failure(Failure::RetryExhausted),
        None,
        None,
    );
    assert!(!contains(&report, b"Diagnostic-Code:"));
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
