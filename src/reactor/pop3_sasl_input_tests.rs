//! Проверки bounded discard для длинных POP3 SASL continuation строк.

use super::{InputProgress, Pop3SaslInput};
use crate::reactor::MAX_INPUT;

#[test]
fn oversized_complete_response_preserves_pipelined_command() {
    let mut input = vec![b'A'; MAX_INPUT.saturating_add(1)];
    input.extend_from_slice(b"\r\nNOOP\r\n");
    let mut state = Pop3SaslInput::default();

    let progress = state.append(&mut input, &[], true);

    assert!(matches!(progress, InputProgress::OversizedComplete));
    assert_eq!(input, b"NOOP\r\n");
}

#[test]
fn oversized_response_is_discarded_across_reads_with_crlf_split() {
    let mut input = vec![b'A'; MAX_INPUT.saturating_add(1)];
    let mut state = Pop3SaslInput::default();
    let first = state.append(&mut input, &[], true);

    assert!(matches!(first, InputProgress::Discarding));
    assert!(input.is_empty());

    let second = state.append(&mut input, b"tail\r", true);
    assert!(matches!(second, InputProgress::Discarding));
    assert!(input.is_empty());

    let final_read = state.append(&mut input, b"\nSTAT\r\n", true);

    assert!(matches!(final_read, InputProgress::OversizedComplete));
    assert_eq!(input, b"STAT\r\n");
}

#[test]
fn other_protocol_input_still_uses_the_general_limit() {
    let mut input = Vec::new();
    let state = &mut Pop3SaslInput::default();
    let progress = state.append(&mut input, &vec![b'X'; MAX_INPUT.saturating_add(1)], false);

    assert!(matches!(progress, InputProgress::Appended));
    assert!(input.len() > MAX_INPUT);
}
