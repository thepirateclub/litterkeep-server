//! Граничные тесты ограниченной SMTP mailbox грамматики.
use super::*;

#[test]
fn accepts_dot_atom_and_dns_domain_labels() {
    for mailbox in ["alice@test", "a.b+tag@sub-domain.example", "x@a"] {
        assert_eq!(classify_mailbox(mailbox), MailboxForm::DotAtom, "{mailbox}");
    }
}

#[test]
fn rejects_empty_or_malformed_dot_atoms() {
    for mailbox in [
        "@test",
        ".alice@test",
        "alice.@test",
        "a..b@test",
        "a@@test",
    ] {
        assert_eq!(classify_mailbox(mailbox), MailboxForm::Invalid, "{mailbox}");
    }
}

#[test]
fn rejects_invalid_domain_labels_and_non_ascii() {
    for mailbox in [
        "alice@-bad.test",
        "alice@bad-.test",
        "alice@a..test",
        "alice@тест",
    ] {
        assert_eq!(classify_mailbox(mailbox), MailboxForm::Invalid, "{mailbox}");
    }
}

#[test]
fn enforces_each_dns_label_octet_limit() {
    let valid = format!("alice@{}", "a".repeat(63));
    let invalid = format!("alice@{}", "a".repeat(64));
    assert_eq!(classify_mailbox(&valid), MailboxForm::DotAtom);
    assert_eq!(classify_mailbox(&invalid), MailboxForm::Invalid);
}

#[test]
fn classifies_bare_postmaster_case_insensitively() {
    for mailbox in ["Postmaster", "POSTMASTER", "postmaster"] {
        assert_eq!(classify_mailbox(mailbox), MailboxForm::BarePostmaster);
    }
    assert_eq!(classify_mailbox(""), MailboxForm::NullReversePath);
}

#[test]
fn accepts_and_canonicalizes_rfc_quoted_local_parts() {
    for mailbox in ["\"alice\"@test", "\"a\\@b\"@test", "\"a>b\"@test"] {
        assert_eq!(classify_mailbox(mailbox), MailboxForm::QuotedLocalPart);
    }
    assert_eq!(
        canonical_parts("\"alice\"@LITTERKEEP.TEST"),
        Some(("alice".to_owned(), "LITTERKEEP.TEST"))
    );
    assert_eq!(
        canonical_parts("\"a\\@b\"@test"),
        Some(("a@b".to_owned(), "test"))
    );
    assert_eq!(
        canonical_parts("\"a\\\"b\"@test"),
        Some(("a\"b".to_owned(), "test"))
    );
    assert_eq!(
        canonical_parts("\"a>b\"@test"),
        Some(("a>b".to_owned(), "test"))
    );
}

#[test]
fn rejects_malformed_or_oversized_quoted_local_parts() {
    for mailbox in [
        "\"unterminated@test",
        "\"dangling\\\"@test",
        "\"control\x01octet\"@test",
        "\"non-ascii-é\"@test",
        "\"a\"@test@other",
    ] {
        assert_eq!(
            classify_mailbox(mailbox),
            MailboxForm::Invalid,
            "{mailbox:?}"
        );
    }
    assert!(canonical_local_part(&format!("\"{}\"", "a".repeat(62))).is_some());
    assert!(canonical_local_part(&format!("\"{}\"", "a".repeat(63))).is_none());
}

#[test]
fn validates_and_discards_rfc_source_routes() {
    assert_eq!(
        without_source_route("@first.test,@relay.test:\"alice\"@test"),
        Some("\"alice\"@test")
    );
    assert_eq!(without_source_route("alice@test"), Some("alice@test"));
    for invalid in [
        "@first.test,,@relay.test:alice@test",
        "@first.test,relay.test:alice@test",
        "@first..test:alice@test",
        "@first.test alice@test",
    ] {
        assert_eq!(without_source_route(invalid), None, "{invalid}");
    }
}
