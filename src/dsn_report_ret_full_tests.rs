//! Проверка RET=FULL для полной исходной MIME-сущности.

use super::*;

/// RET=FULL включает исходный MIME payload только в failure DSN.
#[test]
fn full_return_policy_preserves_the_complete_source_message() {
    let metadata = DsnDeliveryMetadata {
        mail: MailDsn {
            return_content: Some(ReturnContent::Full),
            ..MailDsn::default()
        },
        recipient: None,
        origin_recipients: Vec::new(),
    };
    let source = b"Subject: full return\r\nContent-Type: text/plain\r\n\r\ncomplete body bytes";
    let report = render(
        "alice@litterkeep.test",
        "bob@peer.test",
        29,
        source,
        ReportOutcome::Failure(Failure::PermanentSmtp),
        Some(&metadata),
    );
    assert!(contains(&report, b"Content-Type: message/rfc822\r\n\r\n"));
    assert!(contains(&report, source));
}
