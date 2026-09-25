//! Проверяет MIME entity из Sieve vacation до сохранения script.

use crate::mime::{parameters::MediaType, transfer::TransferEncoding};

/// Проверяет структуру заголовков и wire-кодировку entity согласно RFC 2045.
///
/// В ответе запрещён 8BIT/BINARY, потому что исходящий transport сейчас
/// принимает только SMTP 7BIT. Неоднозначные повторные CTE также не могут
/// безопасно описывать один body и отклоняются до сохранения Sieve script.
pub(super) fn validate_reason(entity: &[u8]) -> Result<(), &'static str> {
    let parsed = crate::mime::header_block::parse(entity)?;
    let header_end = parsed
        .body_offset
        .checked_sub(2)
        .ok_or("invalid MIME boundary")?;
    let headers = entity.get(..header_end).ok_or("invalid MIME headers")?;
    if headers.iter().any(|byte| !byte.is_ascii()) {
        return Err("8bit MIME header");
    }
    for field in &parsed.fields {
        if !super::vacation_message::mime_entity_header(&field.name) {
            return Err("non-MIME entity header");
        }
    }
    validate_mime_version(&parsed.fields)?;
    let media_type = parse_content_type(&parsed.fields)?;
    let fields = parsed
        .fields
        .iter()
        .filter(|field| {
            field.kind() == crate::mime::header_block::HeaderName::ContentTransferEncoding
        })
        .collect::<Vec<_>>();
    let field = match fields.as_slice() {
        [] => None,
        [field] => Some(*field),
        _ => return Err("duplicate MIME transfer encoding"),
    };
    let encoding = match field {
        Some(field) => {
            TransferEncoding::parse(&field.value).map_err(|_| "invalid MIME transfer encoding")?
        }
        None => TransferEncoding::SevenBit,
    };
    let body = entity
        .get(parsed.body_offset..)
        .ok_or("invalid MIME body")?;
    validate_body(&encoding, body, media_type.as_ref())
}

/// Проверяет, что готовое Sieve-сообщение не объявит несовместимую MIME-версию.
fn validate_mime_version(
    fields: &[crate::mime::header_block::HeaderField],
) -> Result<(), &'static str> {
    let mut versions = fields
        .iter()
        .filter(|field| field.kind() == crate::mime::header_block::HeaderName::MimeVersion);
    let field = match versions.next() {
        Some(field) => field,
        None => return Ok(()),
    };
    if versions.next().is_some() {
        return Err("duplicate MIME-Version field");
    }
    let value = std::str::from_utf8(&field.value).map_err(|_| "invalid MIME-Version")?;
    let uncommented = crate::mime::structured_comments::remove(value)?;
    let compact: Vec<u8> = uncommented
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    match MimeVersion::parse(&compact) {
        Some(MimeVersion::OnePointZero) => Ok(()),
        None => Err("unsupported MIME-Version"),
    }
}

/// Поддерживаемая MIME-Version declaration, преобразованная из wire value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MimeVersion {
    OnePointZero,
}

impl MimeVersion {
    fn parse(value: &[u8]) -> Option<Self> {
        match value {
            b"1.0" => Some(Self::OnePointZero),
            _ => None,
        }
    }
}

fn parse_content_type(
    fields: &[crate::mime::header_block::HeaderField],
) -> Result<Option<MediaType>, &'static str> {
    let content_types = fields
        .iter()
        .filter(|field| field.kind() == crate::mime::header_block::HeaderName::ContentType)
        .collect::<Vec<_>>();
    let field = match content_types.as_slice() {
        [] => return Ok(None),
        [field] => *field,
        _ => return Err("duplicate MIME content type"),
    };
    let value = std::str::from_utf8(&field.value).map_err(|_| "invalid MIME content type")?;
    crate::mime::parameters::parse(value).map(|structured| Some(structured.media_type))
}

fn validate_body(
    encoding: &TransferEncoding,
    body: &[u8],
    media_type: Option<&MediaType>,
) -> Result<(), &'static str> {
    // RFC считает длину строк в октетах, а не символах. Ограничения различаются
    // для обычного 7bit и двух кодировок, предназначенных для коротких строк.
    let (maximum_line, rule) = match (encoding, composite(media_type)) {
        (TransferEncoding::SevenBit, _) => (998, BodyOctetRule::SevenBit),
        (TransferEncoding::Base64, false) => (76, BodyOctetRule::Base64),
        (TransferEncoding::QuotedPrintable, false) => (76, BodyOctetRule::QuotedPrintable),
        (TransferEncoding::Base64 | TransferEncoding::QuotedPrintable, true) => {
            return Err("encoded transfer is forbidden for composite MIME type");
        }
        (
            TransferEncoding::EightBit
            | TransferEncoding::Binary
            | TransferEncoding::Unsupported(_),
            _,
        ) => return Err("unsupported MIME transfer encoding"),
    };
    validate_lines(body, maximum_line, rule)?;
    match encoding {
        TransferEncoding::QuotedPrintable => {
            crate::mime::quoted_printable::decode_strict(body).map(drop)
        }
        _ => encoding.decode(body).map(drop),
    }
    .map_err(|_| "invalid encoded MIME body")
}

fn composite(media_type: Option<&MediaType>) -> bool {
    match media_type {
        Some(
            MediaType::MultipartMixed
            | MediaType::MultipartAlternative
            | MediaType::MultipartRelated
            | MediaType::MultipartDigest
            | MediaType::MultipartOther(_)
            | MediaType::MessageRfc822
            | MediaType::MessageOther(_),
        ) => true,
        Some(
            MediaType::TextPlain
            | MediaType::TextHtml
            | MediaType::TextOther(_)
            | MediaType::ApplicationOctetStream
            | MediaType::Other(_),
        )
        | None => false,
    }
}

/// За один проход проверяет CRLF, длину строки и алфавит кодировки.
///
/// Cursor всегда указывает на необработанный октет. CRLF потребляется парой,
/// поэтому отдельные CR/LF не смогут незаметно изменить wire-представление.
fn validate_lines(body: &[u8], maximum: usize, rule: BodyOctetRule) -> Result<(), &'static str> {
    let mut cursor = 0_usize;
    let mut line_length = 0_usize;
    let mut last_octet = None;
    while let Some(octet) = body.get(cursor).copied() {
        match octet {
            b'\r' => {
                let Some(b'\n') = body.get(cursor.saturating_add(1)).copied() else {
                    return Err("bare CR in MIME body");
                };
                validate_line_end(last_octet, line_length, maximum, rule)?;
                line_length = 0;
                last_octet = None;
                cursor = cursor.saturating_add(2);
            }
            b'\n' => return Err("bare LF in MIME body"),
            value => {
                if matches!(
                    (rule, value, last_octet),
                    (BodyOctetRule::QuotedPrintable, b'=', Some(b' ' | b'\t'))
                ) {
                    return Err("quoted-printable soft break follows whitespace");
                }
                line_length = line_length.saturating_add(1);
                if line_length > maximum || !rule.accepts(value) {
                    return Err("invalid MIME body line");
                }
                last_octet = Some(value);
                cursor = cursor.saturating_add(1);
            }
        }
    }
    validate_line_end(last_octet, line_length, maximum, rule)
}

fn validate_line_end(
    last_octet: Option<u8>,
    line_length: usize,
    maximum: usize,
    rule: BodyOctetRule,
) -> Result<(), &'static str> {
    match (
        rule,
        line_length > maximum,
        matches!(last_octet, Some(b' ' | b'\t')),
    ) {
        (_, true, _) => Err("MIME body line too long"),
        (BodyOctetRule::QuotedPrintable, false, true) => {
            Err("quoted-printable line ends in whitespace")
        }
        _ => Ok(()),
    }
}

#[derive(Clone, Copy)]
enum BodyOctetRule {
    SevenBit,
    Base64,
    QuotedPrintable,
}

impl BodyOctetRule {
    fn accepts(self, octet: u8) -> bool {
        match self {
            Self::SevenBit => octet.is_ascii() && octet != 0,
            Self::Base64 => {
                matches!(octet, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' | b'=')
            }
            Self::QuotedPrintable => {
                matches!(octet, b' ' | b'\t' | b'!'..=b'<' | b'=' | b'>'..=b'~')
            }
        }
    }
}
