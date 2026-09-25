//! Рекурсивное MIME-дерево с абсолютными диапазонами исходного RFC822.

use super::header_block::{self, HeaderField};
use super::parameters::MediaType;
use super::transfer::TransferEncoding;
#[path = "tree_content_type.rs"]
mod content_type;
#[cfg(test)]
#[path = "tree_digest_tests.rs"]
mod digest_tests;
#[path = "tree_multipart.rs"]
mod multipart;
#[path = "tree_transfer.rs"]
mod transfer_policy;
#[cfg(test)]
#[path = "tree_transport_tests.rs"]
mod transport_tests;
use multipart::{absolute, is_multipart, multipart_ranges};
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
    )
}

fn parse_entity(
    raw: &[u8],
    base: usize,
    depth: usize,
    count: &mut usize,
    default_media_type: content_type::DefaultMediaType,
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
            let ranges = multipart_ranges(body, boundary.as_bytes(), remaining_parts)?;
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
    fn mime_base64_ignores_non_alphabet_octets() {
        let raw = b"Content-Type: application/octet-stream\r\nContent-Transfer-Encoding: base64\r\n\r\n!aGVsbG8=";
        let entity = parse(raw).expect("MIME entity");

        assert_eq!(entity.decoded_body(raw).expect("decoded body"), b"hello");
    }

    #[test]
    fn message_rfc822_contains_nested_entity_with_absolute_ranges() {
        let raw = b"Content-Type: message/rfc822\r\n\r\nSubject: nested\r\nContent-Type: text/plain\r\n\r\nbody";
        let entity = parse(raw).expect("message entity");
        let nested = entity.children.first().expect("nested entity");
        assert_eq!(raw.get(nested.body_range.clone()), Some(b"body".as_slice()));
    }

    #[test]
    fn rejects_transforming_and_unknown_cte_on_composite_entities() {
        for content_type in ["multipart/mixed; boundary=x", "message/partial"] {
            for transfer_encoding in ["base64", "quoted-printable", "x-private"] {
                let raw = format!(
                    "Content-Type: {content_type}\r\nContent-Transfer-Encoding: {transfer_encoding}\r\n\r\nbody"
                );
                assert!(
                    parse(raw.as_bytes()).is_err(),
                    "accepted {content_type} with {transfer_encoding}"
                );
            }
        }

        let duplicate = b"Content-Type: multipart/mixed; boundary=x\r\nContent-Transfer-Encoding: base64\r\nContent-Transfer-Encoding: 8bit\r\n\r\nbody";
        assert!(parse(duplicate).is_err());
    }

    #[test]
    fn permits_identity_cte_on_multipart_entity() {
        let raw = b"Content-Type: multipart/mixed; boundary=x\r\nContent-Transfer-Encoding: 8bit\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\nbody\r\n--x--\r\n";

        assert!(parse(raw).is_ok());
    }

    #[test]
    fn unknown_leaf_encoding_is_opaque_without_rewriting_source_headers() {
        let raw = b"Content-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: x-private\r\n\r\nbody";
        let entity = parse(raw).expect("opaque MIME leaf");

        assert_eq!(entity.media_type, MediaType::ApplicationOctetStream);
        assert!(entity.parameters.is_empty());
        assert_eq!(
            entity.headers.first().map(|field| field.value.as_slice()),
            Some(b"text/plain; charset=utf-8".as_slice())
        );
        assert_eq!(raw.get(entity.body_range), Some(b"body".as_slice()));
    }

    #[test]
    fn duplicate_cte_fields_are_rejected_instead_of_selecting_the_last() {
        let raw = b"Content-Type: text/plain\r\nContent-Transfer-Encoding: x-private\r\nContent-Transfer-Encoding: 8bit\r\n\r\nbody";

        assert!(parse(raw).is_err());
    }

    #[test]
    fn malformed_content_type_falls_back_to_plain_text_and_keeps_header() {
        let raw = b"Content-Type: text/plain; charset=\"unterminated\r\n\r\nplain body";
        let entity = parse(raw).expect("RFC 2045 default type");

        assert_eq!(entity.media_type, MediaType::TextPlain);
        assert!(entity.parameters.is_empty());
        assert_eq!(raw.get(entity.body_range), Some(b"plain body".as_slice()));
        assert_eq!(
            entity.headers.first().map(|field| field.value.as_slice()),
            Some(b"text/plain; charset=\"unterminated".as_slice())
        );
    }

    #[test]
    fn duplicate_content_type_fields_are_rejected_as_ambiguous() {
        let raw = b"Content-Type: text/plain\r\nContent-Type: application/octet-stream\r\n\r\nbody";

        assert!(parse(raw).is_err());
    }

    #[test]
    fn multipart_rejects_a_boundary_longer_than_seventy_octets() {
        let boundary = "a".repeat(71);
        let raw = format!(
            "Content-Type: multipart/mixed; boundary={boundary}\r\n\r\n--{boundary}\r\n\r\nbody\r\n--{boundary}--\r\n"
        );

        assert!(parse(raw.as_bytes()).is_err());
    }

    #[test]
    fn multipart_stops_at_the_remaining_global_part_budget() {
        let mut raw = String::from("Content-Type: multipart/mixed; boundary=x\r\n\r\n");
        for _ in 0..256 {
            raw.push_str("--x\r\n\r\npart\r\n");
        }
        raw.push_str("--x--\r\n");

        assert!(parse(raw.as_bytes()).is_err());
    }
}
