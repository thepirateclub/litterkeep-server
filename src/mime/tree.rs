//! Рекурсивное MIME-дерево с абсолютными диапазонами исходного RFC822.

use super::header_block::{self, HeaderName};
use super::parameters::{self, MediaType};
use super::transfer::TransferEncoding;
use std::collections::BTreeMap;
use std::ops::Range;

const MAX_DEPTH: usize = 16;
const MAX_PARTS: usize = 256;

/// MIME entity не копирует тело: все диапазоны указывают в исходный message.
#[derive(Debug, Eq, PartialEq)]
pub struct Entity {
    pub raw_range: Range<usize>,
    pub header_range: Range<usize>,
    pub body_range: Range<usize>,
    pub media_type: MediaType,
    pub parameters: BTreeMap<String, String>,
    pub transfer_encoding: TransferEncoding,
    pub children: Vec<Entity>,
    pub preamble_range: Option<Range<usize>>,
    pub epilogue_range: Option<Range<usize>>,
}

struct Delimiter {
    start: usize,
    after: usize,
    closing: bool,
}

struct MultipartRanges {
    parts: Vec<Range<usize>>,
    preamble: Range<usize>,
    epilogue: Range<usize>,
}

/// Строит bounded дерево за линейный проход каждого уровня вложенности.
pub fn parse(raw: &[u8]) -> Result<Entity, &'static str> {
    let mut count = 0_usize;
    parse_entity(raw, 0, 0, &mut count)
}

fn parse_entity(
    raw: &[u8],
    base: usize,
    depth: usize,
    count: &mut usize,
) -> Result<Entity, &'static str> {
    if depth > MAX_DEPTH {
        return Err("MIME nesting limit exceeded");
    }
    *count = count.checked_add(1).ok_or("MIME part count overflow")?;
    if *count > MAX_PARTS {
        return Err("MIME part limit exceeded");
    }
    let headers = header_block::parse(raw)?;
    let content_type = headers
        .fields
        .iter()
        .find(|field| matches!(field.kind(), HeaderName::ContentType))
        .map(|field| std::str::from_utf8(&field.value))
        .transpose()
        .map_err(|_| "Content-Type is not ASCII compatible")?
        .unwrap_or("text/plain");
    let structured = parameters::parse(content_type)?;
    let transfer_encoding = headers
        .fields
        .iter()
        .find(|field| matches!(field.kind(), HeaderName::ContentTransferEncoding))
        .map(|field| TransferEncoding::parse(&field.value))
        .transpose()?
        .unwrap_or(TransferEncoding::SevenBit);
    let boundary = structured.parameters.get("boundary").cloned();
    let body = raw
        .get(headers.body_offset..)
        .ok_or("invalid MIME body range")?;
    let mut children = Vec::new();
    let mut preamble_range = None;
    let mut epilogue_range = None;
    match &structured.media_type {
        media_type if is_multipart(media_type) => {
            let boundary = boundary.ok_or("multipart boundary is absent")?;
            let ranges = multipart_ranges(body, boundary.as_bytes())?;
            preamble_range = Some(absolute(&ranges.preamble, base + headers.body_offset));
            epilogue_range = Some(absolute(&ranges.epilogue, base + headers.body_offset));
            for range in ranges.parts {
                let part = body.get(range.clone()).ok_or("invalid MIME part range")?;
                children.push(parse_entity(
                    part,
                    base + headers.body_offset + range.start,
                    depth.saturating_add(1),
                    count,
                )?);
            }
        }
        MediaType::MessageRfc822 => {
            match &transfer_encoding {
                TransferEncoding::SevenBit
                | TransferEncoding::EightBit
                | TransferEncoding::Binary => {}
                _ => return Err("encoded message/rfc822 cannot preserve raw child ranges"),
            }
            children.push(parse_entity(
                body,
                base + headers.body_offset,
                depth.saturating_add(1),
                count,
            )?);
        }
        _ => {}
    }
    Ok(Entity {
        raw_range: base..base + raw.len(),
        header_range: base..base + headers.body_offset.saturating_sub(2),
        body_range: base + headers.body_offset..base + raw.len(),
        media_type: structured.media_type,
        parameters: structured.parameters,
        transfer_encoding,
        children,
        preamble_range,
        epilogue_range,
    })
}

impl Entity {
    /// Декодирует только тело leaf entity по сохранённому абсолютному range.
    pub fn decoded_body(&self, message: &[u8]) -> Result<Vec<u8>, &'static str> {
        if !self.children.is_empty() {
            return Err("multipart entity has no scalar decoded body");
        }
        let body = message
            .get(self.body_range.clone())
            .ok_or("entity body range is outside message")?;
        self.transfer_encoding.decode(body)
    }
}

fn is_multipart(media_type: &MediaType) -> bool {
    matches!(
        media_type,
        MediaType::MultipartMixed | MediaType::MultipartAlternative | MediaType::MultipartRelated
    )
}

fn multipart_ranges(body: &[u8], boundary: &[u8]) -> Result<MultipartRanges, &'static str> {
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

fn absolute(range: &Range<usize>, base: usize) -> Range<usize> {
    base + range.start..base + range.end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_substring_does_not_split_and_ranges_preserve_parts() {
        let raw = b"Content-Type: multipart/mixed; boundary=x\r\n\r\npreamble\r\n--x\r\nContent-Type: text/plain\r\n\r\ninside --x text\r\n--x\r\nContent-Type: text/html\r\n\r\n<b>x</b>\r\n--x--\r\nepilogue";
        let tree = parse(raw).expect("multipart");
        assert_eq!(tree.children.len(), 2);
        let first = tree.children.first().expect("first part");
        assert_eq!(
            raw.get(first.body_range.clone()),
            Some(b"inside --x text".as_slice())
        );
        let epilogue = tree.epilogue_range.expect("epilogue");
        assert_eq!(raw.get(epilogue), Some(b"epilogue".as_slice()));
    }

    #[test]
    fn missing_closing_delimiter_is_rejected() {
        let raw = b"Content-Type: multipart/mixed; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\nbody";
        assert!(parse(raw).is_err());
    }

    #[test]
    fn leaf_decodes_only_its_raw_body_range() {
        let raw = b"Content-Type: application/octet-stream\r\nContent-Transfer-Encoding: base64\r\n\r\naGVsbG8A";
        let entity = parse(raw).expect("entity");
        assert_eq!(entity.decoded_body(raw).expect("decoded body"), b"hello\0");
    }

    #[test]
    fn message_rfc822_contains_nested_entity_with_absolute_ranges() {
        let raw = b"Content-Type: message/rfc822\r\n\r\nSubject: nested\r\nContent-Type: text/plain\r\n\r\nbody";
        let entity = parse(raw).expect("message entity");
        let nested = entity.children.first().expect("nested entity");
        assert_eq!(raw.get(nested.body_range.clone()), Some(b"body".as_slice()));
    }
}
