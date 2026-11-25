//! Проверяет expansion и повторную валидацию строк RFC 5230 vacation.

use super::execute_state;
use crate::sieve::{VacationInterval, parse};

#[test]
fn vacation_expands_header_mailbox_identity_and_mime_reason_fields() {
    let source = "require [\"vacation\", \"variables\"]; set \"subject\" \"Gone\"; set \"from\" \"away@example.test\"; set \"address\" \"bob@example.test\"; set \"handle\" \"summer\"; set \"body\" \"Away\"; vacation :subject \"${subject}\" :from \"${from}\" :addresses \"${address}\" :mime :handle \"${handle}\" \"Content-Type: text/plain\r\n\r\n${body}\r\n\";";
    let script = parse(source).expect("dynamic vacation template");
    let (plan, _) = execute_state(&script, &[], 0, "", "").expect("expanded MIME is valid");
    let Some(action) = plan.vacations.first() else {
        panic!("one vacation action is planned");
    };

    assert_eq!(action.subject.as_deref(), Some("Gone"));
    assert_eq!(action.from.as_deref(), Some("away@example.test"));
    assert_eq!(action.addresses, vec!["bob@example.test"]);
    assert_eq!(action.handle.as_deref(), Some("summer"));
    assert_eq!(action.interval, None::<VacationInterval>);
    assert!(action.mime);
    assert_eq!(action.reason, b"Content-Type: text/plain\r\n\r\nAway\r\n");
}

#[test]
fn vacation_revalidates_expanded_from_before_delivery_planning() {
    let source = "require [\"vacation\", \"variables\"]; set \"from\" \"away@example.test\r\nBcc: victim@example.test\"; vacation :from \"${from}\" \"Away\";";
    let script = parse(source).expect("raw template has no header injection");

    assert!(execute_state(&script, &[], 0, "", "").is_err());
}

#[test]
fn vacation_revalidates_expanded_mime_entity() {
    let source = "require [\"vacation\", \"variables\"]; set \"body\" \"Content-Type: text/plain\r\nBcc: victim@example.test\r\n\r\nAway\"; vacation :mime \"${body}\";";
    let script = parse(source).expect("MIME template may be checked at runtime");

    assert!(execute_state(&script, &[], 0, "", "").is_err());
}
