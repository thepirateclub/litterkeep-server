//! Проверки Shift_JIS mappings, private-use extension, aliases и отказов.

use super::super::charset::Charset;
use super::decode;

#[test]
fn decodes_ascii_japanese_kana_and_windows_eudc() {
    assert_eq!(decode(b"Shift_JIS"), Ok("Shift_JIS".to_owned()));
    assert_eq!(decode(b"\x93\xfa\x96\x7b\x8c\xea"), Ok("日本語".to_owned()));
    assert_eq!(decode(b"\xb6\xc0"), Ok("ｶﾀ".to_owned()));
    assert_eq!(
        decode(b"\xf0\x40\xf9\xfc"),
        Ok("\u{e000}\u{e757}".to_owned())
    );
    assert_eq!(decode(b"\x80"), Ok("\u{0080}".to_owned()));
}

#[test]
fn whatwg_and_iana_shift_jis_labels_share_typed_variant() {
    for label in [
        "Shift_JIS",
        "shift-jis",
        "shift_jis",
        "csshiftjis",
        "csShiftJIS",
        "csWindows31J",
        "ms932",
        "ms_kanji",
        "sjis",
        "windows-31j",
        "x-sjis",
    ] {
        assert!(
            matches!(Charset::parse(label), Ok(Charset::ShiftJis)),
            "{label}"
        );
    }
    assert_eq!(Charset::ShiftJis.decode(b"\x93\xfa"), Ok("日".to_owned()));
}

#[test]
fn malformed_truncated_and_unmapped_pairs_fail_closed() {
    for bytes in [
        b"\xa0".as_slice(),
        b"\xfd".as_slice(),
        b"\x81".as_slice(),
        b"\x81\x7f".as_slice(),
        b"\x81\xfd".as_slice(),
        b"\xfc\xfc".as_slice(),
    ] {
        assert!(decode(bytes).is_err(), "accepted {bytes:02x?}");
    }
}

#[test]
fn every_two_byte_pointer_matches_the_pinned_jis_index_or_eudc_range() {
    for pointer in 0..11_280 {
        let lead_offset = pointer / 188;
        let trail_slot = pointer % 188;
        let lead = match lead_offset {
            0..=30 => u8::try_from(0x81 + lead_offset).expect("lead fits u8"),
            _ => u8::try_from(0xe0 + lead_offset - 31).expect("lead fits u8"),
        };
        let trail = match trail_slot {
            0..=62 => u8::try_from(0x40 + trail_slot).expect("trail fits u8"),
            _ => u8::try_from(0x41 + trail_slot).expect("trail fits u8"),
        };
        let expected = match pointer {
            8_836..=10_715 => char::from_u32(0xe000 + (pointer - 8_836) as u32),
            _ => super::super::charset_euc_jp_table::jis0208(pointer),
        }
        .map(String::from);
        assert_eq!(decode(&[lead, trail]).ok(), expected, "pointer {pointer}");
    }
}

#[test]
fn every_single_octet_matches_shift_jis_ranges() {
    for value in 0..=u8::MAX {
        let expected = match value {
            0x00..=0x80 => Some(char::from(value)),
            0xa1..=0xdf => char::from_u32(0xff61 + u32::from(value - 0xa1)),
            _ => None,
        }
        .map(String::from);
        assert_eq!(decode(&[value]).ok(), expected, "octet {value:02x}");
    }
}
