//! Проверки полноты IBM866 по опубликованной таблице Unicode CP866.

use super::decode;
use crate::mime::charset::Charset;

#[test]
/// Сверяет все байты, не используя production lookup и диапазонную логику.
fn every_octet_matches_the_unicode_vendor_table() {
    let decoded = decode(&(0..=u8::MAX).collect::<Vec<_>>());
    let actual = decoded.chars();
    let expected = (0..=u8::MAX).filter_map(reference);
    assert_eq!(actual.collect::<Vec<_>>(), expected.collect::<Vec<_>>());
}

/// Возвращает независимое ожидаемое назначение конкретного CP866 octet.
fn reference(byte: u8) -> Option<char> {
    match byte {
        0x00..=0x7f => Some(char::from(byte)),
        0x80..=0x9f => char::from_u32(0x0410 + u32::from(byte - 0x80)),
        0xa0..=0xaf => char::from_u32(0x0430 + u32::from(byte - 0xa0)),
        0xb0..=0xdf => box_character(byte),
        0xe0..=0xef => char::from_u32(0x0440 + u32::from(byte - 0xe0)),
        0xf0 => Some('Ё'),
        0xf1 => Some('ё'),
        0xf2 => Some('Є'),
        0xf3 => Some('є'),
        0xf4 => Some('Ї'),
        0xf5 => Some('ї'),
        0xf6 => Some('Ў'),
        0xf7 => Some('ў'),
        0xf8 => Some('°'),
        0xf9 => Some('∙'),
        0xfa => Some('·'),
        0xfb => Some('√'),
        0xfc => Some('№'),
        0xfd => Some('¤'),
        0xfe => Some('■'),
        0xff => Some('\u{00a0}'),
    }
}

/// Ожидаемая псевдографика записана отдельными Unicode scalar values.
fn box_character(byte: u8) -> Option<char> {
    match byte {
        0xb0 => Some('░'),
        0xb1 => Some('▒'),
        0xb2 => Some('▓'),
        0xb3 => Some('│'),
        0xb4 => Some('┤'),
        0xb5 => Some('╡'),
        0xb6 => Some('╢'),
        0xb7 => Some('╖'),
        0xb8 => Some('╕'),
        0xb9 => Some('╣'),
        0xba => Some('║'),
        0xbb => Some('╗'),
        0xbc => Some('╝'),
        0xbd => Some('╜'),
        0xbe => Some('╛'),
        0xbf => Some('┐'),
        0xc0 => Some('└'),
        0xc1 => Some('┴'),
        0xc2 => Some('┬'),
        0xc3 => Some('├'),
        0xc4 => Some('─'),
        0xc5 => Some('┼'),
        0xc6 => Some('╞'),
        0xc7 => Some('╟'),
        0xc8 => Some('╚'),
        0xc9 => Some('╔'),
        0xca => Some('╩'),
        0xcb => Some('╦'),
        0xcc => Some('╠'),
        0xcd => Some('═'),
        0xce => Some('╬'),
        0xcf => Some('╧'),
        0xd0 => Some('╨'),
        0xd1 => Some('╤'),
        0xd2 => Some('╥'),
        0xd3 => Some('╙'),
        0xd4 => Some('╘'),
        0xd5 => Some('╒'),
        0xd6 => Some('╓'),
        0xd7 => Some('╫'),
        0xd8 => Some('╪'),
        0xd9 => Some('┘'),
        0xda => Some('┌'),
        0xdb => Some('█'),
        0xdc => Some('▄'),
        0xdd => Some('▌'),
        0xde => Some('▐'),
        0xdf => Some('▀'),
        _ => None,
    }
}

#[test]
/// Проверяет русский текст и отличительные украинские буквы CP866.
fn decodes_cyrillic_and_language_specific_letters() {
    assert_eq!(decode(b"\x80\xa0\xf0\xf1\xf2\xf3"), "АаЁёЄє");
    assert_eq!(decode(b"\xfd\xfe\xff"), "¤■\u{00a0}");
}

#[test]
/// Проверяет зарегистрированные MIME labels на протокольной границе.
fn charset_labels_resolve_to_the_same_typed_profile() {
    for label in ["IBM866", "CP866", "csIBM866", "866"] {
        let decoded = Charset::parse(label).and_then(|charset| charset.decode(b"\x80\xa0\xf0\xf1"));
        assert_eq!(decoded, Ok("АаЁё".to_owned()), "label {label}");
    }
}
