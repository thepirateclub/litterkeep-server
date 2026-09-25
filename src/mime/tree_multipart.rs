//! Линейный разбор границ multipart без копирования содержимого MIME-частей.
use super::super::parameters::MediaType;
use std::ops::Range;

/// Строка разделителя multipart в исходном теле письма.
struct Delimiter {
    start: usize,
    after: usize,
    closing: bool,
}

/// Абсолютно не связанные с сообщением диапазоны до перевода в Entity ranges.
pub(super) struct MultipartRanges {
    pub(super) parts: Vec<Range<usize>>,
    pub(super) preamble: Range<usize>,
    pub(super) epilogue: Range<usize>,
}

/// Проверяет семейство multipart через закрытый тип медиакатегории.
pub(super) fn is_multipart(media_type: &MediaType) -> bool {
    matches!(
        media_type,
        MediaType::MultipartMixed
            | MediaType::MultipartAlternative
            | MediaType::MultipartRelated
            | MediaType::MultipartDigest
            | MediaType::MultipartOther(_)
    )
}

/// Собирает preamble, части и epilogue одним проходом строк тела.
pub(super) fn multipart_ranges(
    body: &[u8],
    boundary: &[u8],
    part_limit: usize,
) -> Result<MultipartRanges, &'static str> {
    validate_boundary(boundary)?;
    let mut delimiters = Vec::new();
    let delimiter_limit = part_limit.saturating_add(1);
    let mut cursor = 0_usize;
    loop {
        let remaining = body.get(cursor..).ok_or("invalid multipart cursor")?;
        let line_length = remaining
            .windows(2)
            .position(|pair| pair == b"\r\n")
            .map(|length| length.saturating_add(2))
            .unwrap_or(remaining.len());
        let after = cursor.saturating_add(line_length);
        let line = body.get(cursor..after).ok_or("invalid multipart line")?;
        if let Some(closing) = delimiter_kind(line, boundary) {
            if delimiters.len() >= delimiter_limit {
                return Err("multipart part limit exceeded");
            }
            delimiters.push(Delimiter {
                start: cursor,
                after,
                closing,
            });
        }
        if after >= body.len() {
            break;
        }
        cursor = after;
    }
    assemble_ranges(body, delimiters)
}

/// Проверяет длину и алфавит boundary по RFC 2046 §5.1.1.
fn validate_boundary(boundary: &[u8]) -> Result<(), &'static str> {
    match (boundary.is_empty(), boundary.len() > 70) {
        (true, _) => return Err("multipart boundary is empty"),
        (_, true) => return Err("multipart boundary exceeds 70 octets"),
        _ => {}
    }
    if boundary.last() == Some(&b' ') {
        return Err("multipart boundary ends with space");
    }
    match boundary.iter().all(is_boundary_octet) {
        true => Ok(()),
        false => Err("multipart boundary contains a forbidden octet"),
    }
}

/// Проверяет один ASCII-октет множества `bchars` из RFC 2046.
fn is_boundary_octet(byte: &u8) -> bool {
    matches!(
        byte,
        b'0'..=b'9'
            | b'A'..=b'Z'
            | b'a'..=b'z'
            | b' '
            | b'\''
            | b'('
            | b')'
            | b'+'
            | b'_'
            | b','
            | b'-'
            | b'.'
            | b'/'
            | b':'
            | b'='
            | b'?'
    )
}

fn delimiter_kind(line: &[u8], boundary: &[u8]) -> Option<bool> {
    let content = line.strip_suffix(b"\r\n").unwrap_or(line);
    let content = trim_transport_padding(content);
    let marker = content.strip_prefix(b"--")?;
    match marker.strip_prefix(boundary) {
        Some(b"") => Some(false),
        Some(b"--") => Some(true),
        Some(_) | None => None,
    }
}

fn trim_transport_padding(mut bytes: &[u8]) -> &[u8] {
    while let Some(last) = bytes.last() {
        match last {
            b' ' | b'\t' => {
                bytes = bytes
                    .get(..bytes.len().saturating_sub(1))
                    .unwrap_or_default()
            }
            _ => break,
        }
    }
    bytes
}

fn assemble_ranges(
    body: &[u8],
    delimiters: Vec<Delimiter>,
) -> Result<MultipartRanges, &'static str> {
    let first = delimiters
        .first()
        .ok_or("multipart opening delimiter is absent")?;
    if first.closing {
        return Err("multipart closes before its first part");
    }
    // CRLF перед первым delimiter принадлежит boundary и не является частью
    // preamble, так же как перед последующими delimiter не принадлежит part.
    let preamble_end = strip_encapsulation_crlf(body, first.start);
    let preamble = 0..preamble_end;
    let mut parts = Vec::new();
    let mut current = None;
    let mut epilogue_start = None;
    for delimiter in delimiters {
        if let Some(start) = current.take() {
            parts.push(start..strip_encapsulation_crlf(body, delimiter.start));
        }
        match delimiter.closing {
            true => {
                epilogue_start = Some(delimiter.after);
                break;
            }
            false => current = Some(delimiter.after),
        }
    }
    let epilogue_start = epilogue_start.ok_or("multipart closing delimiter is absent")?;
    Ok(MultipartRanges {
        parts,
        preamble,
        epilogue: epilogue_start..body.len(),
    })
}

fn strip_encapsulation_crlf(body: &[u8], end: usize) -> usize {
    let Some(start) = end.checked_sub(2) else {
        return end;
    };
    match body.get(start..end) {
        Some(b"\r\n") => start,
        Some(_) | None => end,
    }
}

/// Переводит range относительно multipart body в координаты всего сообщения.
pub(super) fn absolute(range: &Range<usize>, base: usize) -> Range<usize> {
    base + range.start..base + range.end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_enforces_rfc2046_octet_grammar_and_limits() {
        let maximum = vec![b'a'; 70];
        let too_long = vec![b'a'; 71];

        assert!(validate_boundary(b"a'()+_,-./:=? z").is_ok());
        assert!(validate_boundary(&maximum).is_ok());
        assert!(validate_boundary(b"").is_err());
        assert!(validate_boundary(&too_long).is_err());
        assert!(validate_boundary(b"ends-with-space ").is_err());
        assert!(validate_boundary(b"forbidden@").is_err());
        assert!(validate_boundary("не-ascii".as_bytes()).is_err());
    }

    #[test]
    fn transport_padding_and_boundary_prefix_text_preserve_exact_ranges() {
        let body = b"note\r\n--x \t\r\npart\r\n--x-more\r\nstill\r\n--x-- \t\r\nepilogue";
        let ranges = multipart_ranges(body, b"x", 4).expect("multipart ranges");

        assert_eq!(body.get(ranges.preamble), Some(b"note".as_slice()));
        assert_eq!(
            ranges
                .parts
                .first()
                .and_then(|range| body.get(range.clone())),
            Some(b"part\r\n--x-more\r\nstill".as_slice())
        );
        assert_eq!(body.get(ranges.epilogue), Some(b"epilogue".as_slice()));
    }

    #[test]
    fn preamble_keeps_its_own_line_break_before_boundary_break() {
        let body = b"note\r\n\r\n--x\r\npart\r\n--x--\r\n";
        let ranges = multipart_ranges(body, b"x", 2).expect("multipart ranges");

        assert_eq!(body.get(ranges.preamble), Some(b"note\r\n".as_slice()));
    }
}
