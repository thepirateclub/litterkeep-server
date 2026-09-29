//! Декодирование RFC 5228 encoded-character до построения AST.

use super::lexer::Token;

/// Проверяет, требует ли начальная область script расширение.
///
/// Sieve допускает несколько `require` до первой команды. Capability strings
/// регистрозависимы, поэтому сравнение выполняется с точным RFC-именем.
pub(super) fn is_required(tokens: &[Token]) -> bool {
    let mut tokens = tokens.iter();
    loop {
        match tokens.next() {
            Some(Token::Atom(command)) if command.eq_ignore_ascii_case("require") => {
                for token in tokens.by_ref() {
                    match token {
                        Token::String(value) if value == b"encoded-character" => return true,
                        Token::Semicolon => break,
                        _ => {}
                    }
                }
            }
            Some(Token::Atom(_)) | None => return false,
            _ => return false,
        }
    }
}

/// Декodирует только string token-ы, когда capability объявлена.
pub(super) fn decode_tokens(tokens: &mut [Token]) -> Result<(), &'static str> {
    for token in tokens {
        if let Token::String(value) = token {
            *value = decode(value)?;
        }
    }
    Ok(())
}

fn decode(source: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut output = Vec::with_capacity(source.len());
    let mut remaining = source;
    while let Some(position) = marker_position(remaining) {
        output.extend_from_slice(take_prefix(&mut remaining, position));
        let candidate = decode_candidate(remaining)?;
        match candidate {
            Candidate::Decoded(bytes, consumed) => {
                output.extend_from_slice(&bytes);
                let _ = take_prefix(&mut remaining, consumed);
            }
            Candidate::Literal(consumed) => {
                output.extend_from_slice(take_prefix(&mut remaining, consumed));
            }
            Candidate::ScanNext => {
                output.extend_from_slice(take_prefix(&mut remaining, 1));
            }
        }
    }
    output.extend_from_slice(remaining);
    Ok(output)
}

/// Находит ближайший `${hex:` либо `${unicode:` маркер без приоритета вида.
fn marker_position(source: &[u8]) -> Option<usize> {
    source
        .windows(6)
        .enumerate()
        .find_map(|(position, window)| {
            let hex = window.eq_ignore_ascii_case(b"${hex:");
            let unicode = source
                .get(position..)
                .and_then(|remaining| remaining.get(..10))
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"${unicode:"));
            match (hex, unicode) {
                (true, _) | (_, true) => Some(position),
                (false, false) => None,
            }
        })
}

/// Разбирает один кандидат и сохраняет malformed последовательность literal.
/// Ошибка допускается только для грамматически корректного Unicode числа,
/// которое не обозначает scalar value.
fn decode_candidate(source: &[u8]) -> Result<Candidate, &'static str> {
    let kind = match source.get(..6) {
        Some(prefix) if prefix.eq_ignore_ascii_case(b"${hex:") => Encoding::Hex,
        _ => match source.get(..10) {
            Some(prefix) if prefix.eq_ignore_ascii_case(b"${unicode:") => Encoding::Unicode,
            _ => return Ok(Candidate::ScanNext),
        },
    };
    let prefix_len = kind.prefix_len();
    let rest = source.get(prefix_len..).unwrap_or_default();
    let Some(end) = rest.iter().position(|octet| *octet == b'}') else {
        return Ok(Candidate::ScanNext);
    };
    let contents = rest.get(..end).unwrap_or_default();
    let consumed = prefix_len.saturating_add(end).saturating_add(1);
    match kind.decode(contents) {
        Decoded::Value(bytes) => Ok(Candidate::Decoded(bytes, consumed)),
        Decoded::Malformed if has_nested_marker(contents) => Ok(Candidate::ScanNext),
        Decoded::Malformed => Ok(Candidate::Literal(consumed)),
        Decoded::InvalidUnicode => Err("invalid encoded-character Unicode scalar"),
    }
}

fn has_nested_marker(value: &[u8]) -> bool {
    value
        .windows(6)
        .any(|window| window.eq_ignore_ascii_case(b"${hex:"))
        || value
            .windows(10)
            .any(|window| window.eq_ignore_ascii_case(b"${unicode:"))
}

fn take_prefix<'a>(source: &mut &'a [u8], length: usize) -> &'a [u8] {
    let (prefix, suffix) = source.split_at_checked(length).unwrap_or((*source, &[]));
    *source = suffix;
    prefix
}

#[derive(Clone, Copy)]
enum Encoding {
    Hex,
    Unicode,
}

impl Encoding {
    fn prefix_len(self) -> usize {
        match self {
            Self::Hex => 6,
            Self::Unicode => 10,
        }
    }

    fn decode(self, value: &[u8]) -> Decoded {
        match self {
            Self::Hex => decode_hex(value),
            Self::Unicode => decode_unicode(value),
        }
    }
}

enum Decoded {
    Value(Vec<u8>),
    Malformed,
    InvalidUnicode,
}

enum Candidate {
    Decoded(Vec<u8>, usize),
    Literal(usize),
    ScanNext,
}

fn decode_hex(value: &[u8]) -> Decoded {
    let mut bytes = Vec::new();
    let mut digits = Vec::new();
    for octet in value.iter().copied() {
        match octet {
            b' ' | b'\t' | b'\r' | b'\n' => {
                if !flush_hex_pair(&mut digits, &mut bytes) {
                    return Decoded::Malformed;
                }
            }
            hex if hex_value(hex).is_some() => digits.push(hex),
            _ => return Decoded::Malformed,
        }
    }
    if !flush_hex_pair(&mut digits, &mut bytes) {
        return Decoded::Malformed;
    }
    match bytes.is_empty() {
        true => Decoded::Malformed,
        false => Decoded::Value(bytes),
    }
}

/// Завершает одну пару hex-цифр на границе blank, не теряя неполную пару.
fn flush_hex_pair(digits: &mut Vec<u8>, output: &mut Vec<u8>) -> bool {
    match digits.len() {
        0 => true,
        1 => match digits.first().copied().and_then(hex_value) {
            Some(value) => {
                output.push(value);
                digits.clear();
                true
            }
            None => false,
        },
        2 => {
            let mut values = digits.iter().filter_map(|digit| hex_value(*digit));
            match (values.next(), values.next()) {
                (Some(high), Some(low)) => {
                    output.push((high << 4) | low);
                    digits.clear();
                    true
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn decode_unicode(value: &[u8]) -> Decoded {
    let groups: Vec<&[u8]> = value
        .split(|octet| matches!(octet, b' ' | b'\t' | b'\r' | b'\n'))
        .filter(|group| !group.is_empty())
        .collect();
    match groups.is_empty()
        || groups
            .iter()
            .any(|group| group.iter().any(|digit| hex_value(*digit).is_none()))
    {
        true => Decoded::Malformed,
        false => decode_unicode_groups(&groups),
    }
}

/// Декодирует каждую hex-группу в отдельный Unicode scalar по RFC ABNF.
/// Сначала синтаксис всех групп уже проверен, чтобы malformed суффикс не
/// превратил нормативное literal-preservation в ошибку диапазона.
fn decode_unicode_groups(groups: &[&[u8]]) -> Decoded {
    let mut output = Vec::new();
    for group in groups {
        let mut scalar = 0_u32;
        for digit in group.iter().copied() {
            let Some(value) = hex_value(digit) else {
                return Decoded::Malformed;
            };
            let Some(next) = scalar
                .checked_mul(16)
                .and_then(|number| number.checked_add(u32::from(value)))
            else {
                return Decoded::InvalidUnicode;
            };
            scalar = next;
        }
        let Some(character) = char::from_u32(scalar) else {
            return Decoded::InvalidUnicode;
        };
        let mut buffer = [0_u8; 4];
        output.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
    }
    Decoded::Value(output)
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
#[path = "encoded_character_tests.rs"]
mod tests;
