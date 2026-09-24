//! Граничные тесты ограниченной SMTP mailbox грамматики.
use super::*;

#[test]
fn accepts_dot_atom_and_dns_domain_labels() {
    for mailbox in ["alice@test", "a.b+tag@sub-domain.example", "x@a"] {
        assert!(is_dot_atom_mailbox(mailbox), "mailbox: {mailbox}");
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
        assert!(!is_dot_atom_mailbox(mailbox), "mailbox: {mailbox}");
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
        assert!(!is_dot_atom_mailbox(mailbox), "mailbox: {mailbox}");
    }
}

#[test]
fn enforces_each_dns_label_octet_limit() {
    let valid = format!("alice@{}", "a".repeat(63));
    let invalid = format!("alice@{}", "a".repeat(64));
    assert!(is_dot_atom_mailbox(&valid));
    assert!(!is_dot_atom_mailbox(&invalid));
}
