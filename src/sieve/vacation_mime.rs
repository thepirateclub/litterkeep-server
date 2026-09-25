//! Проверяет MIME entity из Sieve vacation до сохранения script.

use crate::mime::transfer::TransferEncoding;

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
    let fields = parsed
        .fields
        .iter()
        .filter(|field| field.name == "content-transfer-encoding")
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
    validate_body(&encoding, body)
}

fn validate_body(encoding: &TransferEncoding, body: &[u8]) -> Result<(), &'static str> {
    // RFC считает длину строк в октетах, а не символах. Ограничения различаются
    // для обычного 7bit и двух кодировок, предназначенных для коротких строк.
    let (maximum_line, rule) = match encoding {
        TransferEncoding::SevenBit => (998, BodyOctetRule::SevenBit),
        TransferEncoding::Base64 => (76, BodyOctetRule::Base64),
        TransferEncoding::QuotedPrintable => (76, BodyOctetRule::QuotedPrintable),
        TransferEncoding::EightBit
        | TransferEncoding::Binary
        | TransferEncoding::Unsupported(_) => return Err("unsupported MIME transfer encoding"),
    };
    validate_lines(body, maximum_line, rule)?;
    encoding
        .decode(body)
        .map(drop)
        .map_err(|_| "invalid encoded MIME body")
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
