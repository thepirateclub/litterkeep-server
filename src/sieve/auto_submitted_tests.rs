//! Проверки fail-closed поведения Auto-Submitted для RFC 5230 vacation.

/// Unicode whitespace не становится RFC 3834 CFWS и не разрешает автоответ.
#[test]
fn malformed_unicode_whitespace_does_not_disable_auto_submitted_suppression() {
    for value in ["\u{00a0}no", "no\u{00a0}"] {
        assert!(
            !super::may_respond(
                "alice@peer.test",
                &[("auto-submitted".to_owned(), value.to_owned())],
            ),
            "malformed Auto-Submitted value was treated as no: {value:?}"
        );
    }
    assert!(super::may_respond(
        "alice@peer.test",
        &[("auto-submitted".to_owned(), " \tno\t ".to_owned())],
    ));
}

/// RFC 3834 CFWS и параметры не меняют значение keyword `no`.
#[test]
fn recognizes_no_with_comments_and_optional_parameters() {
    for value in [
        " (human origin) no (manual) ",
        "no; x-source=manual",
        "no (origin) ; x-label=\"human; authored\"",
        "no; x-label=\"human\tauthored\"",
    ] {
        assert!(
            super::may_respond(
                "alice@peer.test",
                &[("auto-submitted".to_owned(), value.to_owned())],
            ),
            "valid human Auto-Submitted value was suppressed: {value:?}"
        );
    }
}

/// Malformed parameters и повторные поля остаются fail-closed.
#[test]
fn suppresses_malformed_parameters_and_duplicate_fields() {
    for headers in [
        vec![("auto-submitted".to_owned(), "no; x-broken=".to_owned())],
        vec![
            ("auto-submitted".to_owned(), "no".to_owned()),
            ("auto-submitted".to_owned(), "no".to_owned()),
        ],
    ] {
        assert!(!super::may_respond("alice@peer.test", &headers));
    }

    for value in [
        "auto-generated; x-source=cron",
        "auto-replied; x-source=vacation",
        "x-custom-automation",
    ] {
        assert!(
            !super::may_respond(
                "alice@peer.test",
                &[("auto-submitted".to_owned(), value.to_owned())],
            ),
            "automated Auto-Submitted keyword was allowed: {value:?}"
        );
    }
}
