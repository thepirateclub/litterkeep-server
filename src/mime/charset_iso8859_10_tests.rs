//! Проверки полного Latin-6 mapping по каждому octet.

use super::decode;

#[test]
fn maps_every_octet_to_the_unicode_reference_profile() {
    let bytes: Vec<u8> = (0..=u8::MAX).collect();
    let decoded = decode(&bytes).expect("all ISO-8859-10 octets are assigned");
    assert_eq!(decoded.chars().count(), 256);
    assert_eq!(decoded.chars().next_back(), Some('\u{0138}'));
    assert!(decoded.contains('\u{0160}'));
    assert!(decoded.contains('\u{00f0}'));
}

#[test]
fn preserves_ascii_and_c1_controls() {
    let bytes: Vec<u8> = (0..=0x9f).collect();
    let expected: String = bytes.iter().copied().map(char::from).collect();
    assert_eq!(decode(&bytes), Ok(expected));
}

#[test]
fn decodes_nordic_sample() {
    assert_eq!(
        decode(b"Isl\xe1nd er fallegt"),
        Ok("Islánd er fallegt".to_owned())
    );
}
