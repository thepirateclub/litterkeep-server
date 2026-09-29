//! Проверки длинных POP3 SASL continuation и пути нехватки памяти.

use super::{InputProgress, Pop3SaslInput};
use crate::reactor::MAX_INPUT;

#[test]
fn sasl_response_can_exceed_the_general_session_input_budget() {
    let mut input = Vec::new();
    let mut received = vec![b'A'; MAX_INPUT.saturating_add(1)];
    received.extend_from_slice(b"\r\n");
    let mut state = Pop3SaslInput::default();

    let progress = state.append(&mut input, &received, true);

    assert!(matches!(progress, InputProgress::Appended));
    assert_eq!(input.len(), received.len());
}

#[test]
fn allocation_failure_discards_until_crlf_and_preserves_following_command() {
    let mut input = Vec::new();
    let mut state = Pop3SaslInput {
        discarding: true,
        previous_was_carriage_return: false,
    };

    let first = state.append(&mut input, b"partial\r", true);
    assert!(matches!(first, InputProgress::Discarding));
    assert!(input.is_empty());

    let final_read = state.append(&mut input, b"\nSTAT\r\n", true);

    assert!(matches!(
        final_read,
        InputProgress::AllocationFailureComplete
    ));
    assert_eq!(input, b"STAT\r\n");
}

#[test]
fn other_protocol_input_keeps_the_general_budget_owner() {
    let mut input = Vec::new();
    let state = &mut Pop3SaslInput::default();
    let progress = state.append(&mut input, &vec![b'X'; MAX_INPUT.saturating_add(1)], false);

    assert!(matches!(progress, InputProgress::Appended));
    assert!(input.len() > MAX_INPUT);
}
