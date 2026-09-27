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
fn rejects_invalid_domain_labels_and_invalid_idna_names() {
    for mailbox in [
        "alice@-bad.test",
        "alice@bad-.test",
        "alice@a..test",
        "alice@xn--",
        "alice@xn--abc",
        "alice@123.xn--mgbh0fb",
    ] {
        assert_eq!(classify_mailbox(mailbox), MailboxForm::Invalid, "{mailbox}");
    }
}

#[test]
fn accepts_only_valid_idna_a_labels_in_ascii_domains() {
    assert_eq!(
        classify_mailbox("alice@xn--bcher-kva.example"),
        MailboxForm::DotAtom
    );
}

#[test]
fn requires_smtputf8_for_utf8_local_parts_or_u_label_domains() {
    for mailbox in ["алиса@test", "δοκιμή@example.test", "\"почта\"@test"] {
        assert_ne!(classify_mailbox(mailbox), MailboxForm::Invalid, "{mailbox}");
        assert!(requires_smtp_utf8(mailbox));
    }
    assert_ne!(classify_mailbox("alice@тест"), MailboxForm::Invalid);
    assert_ne!(classify_mailbox("alice@例え.test"), MailboxForm::Invalid);
    assert!(requires_smtp_utf8("alice@тест"));
    assert!(requires_smtp_utf8("alice@例え.test"));
    assert!(!requires_smtp_utf8("alice@test"));
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
        Some(("alice".to_owned(), "litterkeep.test".to_owned()))
    );
    assert_eq!(
        canonical_parts("\"a\\@b\"@test"),
        Some(("a@b".to_owned(), "test".to_owned()))
    );
    assert_eq!(
        canonical_parts("\"a\\\"b\"@test"),
        Some(("a\"b".to_owned(), "test".to_owned()))
    );
    assert_eq!(
        canonical_parts("\"a>b\"@test"),
        Some(("a>b".to_owned(), "test".to_owned()))
    );
}

#[test]
fn accepts_rfc_ipv4_and_tagged_ipv6_address_literals() {
    for mailbox in [
        "alice@[192.0.2.1]",
        "alice@[001.002.003.004]",
        "\"alice\"@[255.255.255.255]",
        "alice@[IPv6:2001:db8::1]",
        "\"alice\"@[ipv6:::ffff:192.0.2.1]",
        "alice@[IPv6:::ffff:192.000.2.1]",
        "alice@[IPv6:::ffff:192.000.2.1]",
    ] {
        assert_eq!(
            classify_mailbox(mailbox),
            MailboxForm::AddressLiteral,
            "{mailbox}"
        );
    }
    assert_eq!(
        canonical_parts("\"alice\"@[IPv6:2001:db8::1]"),
        Some(("alice".to_owned(), "[IPv6:2001:db8::1]".to_owned()))
    );
}

#[test]
fn canonicalizes_u_label_domain_to_a_label_for_local_route_lookup() {
    assert_eq!(
        canonical_parts("alice@bücher.example"),
        Some(("alice".to_owned(), "xn--bcher-kva.example".to_owned()))
    );
    assert!(requires_smtp_utf8("alice@bücher.example"));
}

#[test]
fn rejects_malformed_or_oversized_quoted_local_parts() {
    for mailbox in [
        "\"unterminated@test",
        "\"dangling\\\"@test",
        "\"control\x01octet\"@test",
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
