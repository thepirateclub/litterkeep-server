//! Проверки допустимых символов RFC 5322 dtext и RFC 6532 UTF-8 расширения.

/// Проверяет один символ литерала или obsolete quoted-pair.
///
/// RFC 5322 разрешает в `dtext` видимые ASCII, кроме скобок литерала и
/// обратной косой черты, а также obsolete control bytes. RFC 6532 добавляет
/// UTF-8 символы вне ASCII. Для obsolete quoted-pair допустимы WSP и VCHAR.
pub(super) fn validate(character: char, quoted_pair: bool) -> Result<(), &'static str> {
    let codepoint = character as u32;
    match (quoted_pair, codepoint) {
        (true, 9 | 32 | 33..=126) => Ok(()),
        (false, 128..) => Ok(()),
        (false, 9 | 32 | 33..=90 | 94..=126 | 1..=8 | 11 | 12 | 14..=31 | 127) => Ok(()),
        _ => Err("invalid domain-literal dtext"),
    }
}
