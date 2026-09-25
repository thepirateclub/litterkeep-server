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
            | MediaType::MultipartOther(_)
    )
}

/// Собирает preamble, части и epilogue одним проходом строк тела.
pub(super) fn multipart_ranges(
    body: &[u8],
    boundary: &[u8],
) -> Result<MultipartRanges, &'static str> {
    let mut delimiters = Vec::new();
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
    let preamble = 0..first.start;
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
