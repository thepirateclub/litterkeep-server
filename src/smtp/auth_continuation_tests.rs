//! Регрессии выбора между AUTH response и SMTP command interruption.
use super::{interrupts_exchange, is_nested_auth};

#[test]
fn cancel_is_distinct_from_command_interruption() {
    assert!(!interrupts_exchange(b"*\r\n"));
    assert!(interrupts_exchange(b"NOOP\r\n"));
}

#[test]
fn nested_auth_is_not_treated_as_a_sasl_payload() {
    assert!(interrupts_exchange(b"AUTH PLAIN\r\n"));
    assert!(is_nested_auth(b"AUTH PLAIN\r\n"));
    assert!(is_nested_auth(b"aUtH other\r\n"));
}

#[test]
fn command_arguments_do_not_change_the_verb_classification() {
    assert!(interrupts_exchange(b"NOOP diagnostic text\r\n"));
    assert!(!interrupts_exchange(b"%%%\r\n"));
}

#[test]
fn malformed_line_termination_is_not_a_command_interruption() {
    assert!(!interrupts_exchange(b"NOOP\n"));
}
