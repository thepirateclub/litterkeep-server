//! Проверки RFC 9394 range grammar и границ частичной выборки.

use super::*;

#[test]
fn oldest_range_is_inclusive_and_clamped_to_available_results() {
    let range = PartialRange::parse("2:4").expect("partial range");
    assert_eq!(range.positions(8), 1..4);
    assert_eq!(range.positions(3), 1..3);
    assert_eq!(range.positions(0), 0..0);
}

#[test]
fn newest_range_counts_backwards_but_keeps_mailbox_order() {
    let range = PartialRange::parse("-1:-3").expect("partial range");
    assert_eq!(range.positions(8), 5..8);
    assert_eq!(PartialRange::parse("-3:-1").unwrap().positions(8), 5..8);
    assert_eq!(range.positions(2), 0..2);
}

#[test]
fn rejects_zero_mixed_directions_and_malformed_ranges() {
    for value in ["0:1", "01:2", "-0:-1", "1:-2", "-1:2", "1:*", "1:2:3"] {
        assert!(PartialRange::parse(value).is_none(), "accepted {value}");
    }
}
