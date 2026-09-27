//! Таблица MIME charset ISO-8859-16 (Latin-10).

/// Декодирует все октеты ISO-8859-16 по опубликованной Unicode mapping.
///
/// Румынские и центральноевропейские буквы занимают позиции прежних знаков
/// Latin-1. C0 и C1 остаются соответствующими Unicode control scalars.
pub(super) fn decode(bytes: &[u8]) -> String {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Преобразует один октет, не индексируя входной буфер или таблицу.
fn decode_byte(byte: u8) -> char {
    match byte {
        0xa1 => 'Ą',
        0xa2 => 'ą',
        0xa3 => 'Ł',
        0xa4 => '€',
        0xa5 => '„',
        0xa6 => 'Š',
        0xa8 => 'š',
        0xaa => 'Ș',
        0xac => 'Ź',
        0xae => 'ź',
        0xaf => 'Ż',
        0xb2 => 'Č',
        0xb3 => 'ł',
        0xb4 => 'Ž',
        0xb5 => '”',
        0xb8 => 'ž',
        0xb9 => 'č',
        0xba => 'ș',
        0xbc => 'Œ',
        0xbd => 'œ',
        0xbe => 'Ÿ',
        0xbf => 'ż',
        0xc3 => 'Ă',
        0xc5 => 'Ć',
        0xd0 => 'Đ',
        0xd1 => 'Ń',
        0xd5 => 'Ő',
        0xd7 => 'Ś',
        0xd8 => 'Ű',
        0xdd => 'Ę',
        0xde => 'Ț',
        0xe3 => 'ă',
        0xe5 => 'ć',
        0xf0 => 'đ',
        0xf1 => 'ń',
        0xf5 => 'ő',
        0xf7 => 'ś',
        0xf8 => 'ű',
        0xfd => 'ę',
        0xfe => 'ț',
        _ => char::from(byte),
    }
}

#[cfg(test)]
#[path = "charset_iso8859_16_tests.rs"]
mod tests;
