//! RFC 3030 BDAT reply codes are classified by exact protocol phase.
use crate::outbound::{DeliveryOutcome, ReplyExpectation, ReplyResult};
use std::io::Cursor;

/// RFC 3030 chunk требует точный 250, а 4yz/5yz сохраняют семантику очереди.
#[test]
fn bdat_requires_exact_250_and_classifies_chunk_failures() {
    let mut accepted = Cursor::new(b"250 chunk accepted\r\n");
    assert_eq!(
        crate::outbound::expect(&mut accepted, ReplyExpectation::Bdat).expect("BDAT 250"),
        ReplyResult::Accepted
    );

    let mut deferred = Cursor::new(b"451 retry chunk\r\n");
    assert_eq!(
        crate::outbound::expect(&mut deferred, ReplyExpectation::Bdat).expect("BDAT 451"),
        ReplyResult::Rejected(DeliveryOutcome::Deferred)
    );

    let mut permanent = Cursor::new(b"552 chunk rejected\r\n");
    assert_eq!(
        crate::outbound::expect(&mut permanent, ReplyExpectation::Bdat).expect("BDAT 552"),
        ReplyResult::Rejected(DeliveryOutcome::PermanentFailure)
    );

    let mut unexpected = Cursor::new(b"251 nonconforming chunk response\r\n");
    assert!(crate::outbound::expect(&mut unexpected, ReplyExpectation::Bdat).is_err());
}
