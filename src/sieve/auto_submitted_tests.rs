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
