//! Проверки сериализации RFC 9394 partial ESEARCH result data.

use super::*;

#[test]
fn partial_response_uses_result_positions_and_compact_sequence_sets() {
    let mut fields = Vec::new();
    let range = crate::imap::partial_range::PartialRange::parse("2:4").unwrap();
    append_item(&mut fields, ReturnItem::Partial(range), &[2, 4, 7, 8, 9]);
    assert_eq!(fields, vec!["PARTIAL (2:4 4,7:8)"]);
}

#[test]
fn out_of_range_partial_response_uses_nil() {
    let mut fields = Vec::new();
    let range = crate::imap::partial_range::PartialRange::parse("-8:-9").unwrap();
    append_item(&mut fields, ReturnItem::Partial(range), &[2, 4, 7]);
    assert_eq!(fields, vec!["PARTIAL (-8:-9 NIL)"]);
}
