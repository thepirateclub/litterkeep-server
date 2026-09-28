//! Сверяет все wire pointers с закреплённым WHATWG EUC-KR index.

use super::{charset_euc_kr, charset_euc_kr_table};

#[test]
fn every_two_byte_pointer_matches_its_lead_and_trail_pair() {
    for pointer in 0..charset_euc_kr_table::POINTER_COUNT {
        let leading = u8::try_from(pointer / 190).expect("leading offset") + 0x81;
        let trailing = u8::try_from(pointer % 190).expect("trailing offset") + 0x41;
        let wire = [leading, trailing];
        let expected = charset_euc_kr_table::code_point(pointer).map(String::from);
        assert_eq!(
            charset_euc_kr::decode(&wire).ok(),
            expected,
            "pointer {pointer}"
        );
    }
}
