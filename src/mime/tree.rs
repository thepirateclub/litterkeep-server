//! Рекурсивное MIME-дерево с абсолютными диапазонами исходного RFC822.

use super::header_block::{self, HeaderField};
use super::parameters::MediaType;
use super::transfer::TransferEncoding;
#[path = "tree_content_type.rs"]
mod content_type;
#[cfg(test)]
#[path = "tree_digest_tests.rs"]
mod digest_tests;
#[cfg(test)]
#[path = "tree_entity_tests.rs"]
mod entity_tests;
#[path = "tree_multipart.rs"]
mod multipart;
#[path = "tree_transfer.rs"]
mod transfer_policy;
#[cfg(test)]
#[path = "tree_transport_tests.rs"]
mod transport_tests;
use multipart::{CloseDelimiterPolicy, absolute, is_multipart, multipart_ranges};
use std::collections::BTreeMap;
use std::ops::Range;

const MAX_DEPTH: usize = 16;
const MAX_PARTS: usize = 256;

/// MIME entity не копирует тело: все диапазоны указывают в исходный message.
#[derive(Debug, Eq, PartialEq)]
pub struct Entity {
    /// MIME-заголовки entity после unfold без повторного чтения большого body.
    pub headers: Vec<HeaderField>,
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

/// Строит bounded дерево за линейный проход каждого уровня вложенности.
pub fn parse(raw: &[u8]) -> Result<Entity, &'static str> {
    let mut count = 0_usize;
    parse_entity(
        raw,
        0,
        0,
        &mut count,
        content_type::DefaultMediaType::PlainText,
        CloseDelimiterPolicy::Required,
    )
}

fn parse_entity(
    raw: &[u8],
    base: usize,
    depth: usize,
    count: &mut usize,
    default_media_type: content_type::DefaultMediaType,
    close_policy: CloseDelimiterPolicy,
) -> Result<Entity, &'static str> {
    if depth > MAX_DEPTH {
        return Err("MIME nesting limit exceeded");
    }
    *count = count.checked_add(1).ok_or("MIME part count overflow")?;
    if *count > MAX_PARTS {
        return Err("MIME part limit exceeded");
    }
    let headers = header_block::parse(raw)?;
    let mut structured = content_type::parse(&headers.fields, default_media_type)?;
    let transfer_encoding = transfer_policy::resolve(&headers.fields, &structured.media_type)?;
    // RFC 2045 §6.4 делает entity с неизвестным CTE непрозрачной для
    // MIME-интерпретации: фактический тип становится application/octet-stream.
    // Исходные заголовки остаются в entity.headers и не меняются на wire.
    if let TransferEncoding::Unsupported(_) = transfer_encoding {
        structured.media_type = MediaType::ApplicationOctetStream;
        structured.parameters.clear();
    }
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
            let remaining_parts = MAX_PARTS.saturating_sub(*count);
            let ranges =
                multipart_ranges(body, boundary.as_bytes(), remaining_parts, close_policy)?;
            preamble_range = Some(absolute(&ranges.preamble, base + headers.body_offset));
            epilogue_range = Some(absolute(&ranges.epilogue, base + headers.body_offset));
            for range in ranges.parts {
                let part = body.get(range.clone()).ok_or("invalid MIME part range")?;
                children.push(parse_entity(
                    part,
                    base + headers.body_offset + range.start,
                    depth.saturating_add(1),
                    count,
                    match &structured.media_type {
                        MediaType::MultipartDigest => content_type::DefaultMediaType::MessageRfc822,
                        _ => content_type::DefaultMediaType::PlainText,
                    },
                    CloseDelimiterPolicy::MayEndAtParentBoundary,
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
                content_type::DefaultMediaType::PlainText,
                close_policy,
            )?);
        }
        _ => {}
    }
    Ok(Entity {
        headers: headers.fields,
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
    /// Проверяет CTE binary во всём bounded MIME поддереве, включая вложенные parts.
    pub fn contains_binary_transfer_encoding(&self) -> bool {
        match (
            matches!(self.transfer_encoding, TransferEncoding::Binary),
            self.children
                .iter()
                .any(Entity::contains_binary_transfer_encoding),
        ) {
            (true, _) | (_, true) => true,
            (false, false) => false,
        }
    }

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
