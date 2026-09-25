//! Формирует RFC 5322 response headers и MIME body для vacation.

use super::ast::VacationAction;
use std::io;

/// Создаёт отдельный RFC822 payload с RFC 5230 response fields.
pub(crate) fn build_response(
    action: &VacationAction,
    owner: &str,
    sender: &str,
    headers: &[(String, String)],
    now_epoch: u64,
) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    write_header(
        &mut output,
        "Date",
        &crate::smtp_trace::rfc5322_utc(now_epoch),
    );
    let subject = response_subject(action, headers);
    write_header(&mut output, "Subject", &encode_subject(&subject));
    write_header(&mut output, "From", action.from.as_deref().unwrap_or(owner));
    write_header(&mut output, "To", sender);
    write_header(&mut output, "Auto-Submitted", "auto-replied");
    append_reply_references(&mut output, headers);
    match action.mime {
        true => append_mime_entity(&mut output, action.reason.as_bytes())?,
        false => append_utf8_text(&mut output, action.reason.as_bytes()),
    }
    Ok(output)
}

fn response_subject(action: &VacationAction, headers: &[(String, String)]) -> String {
    match action.subject.as_deref() {
        Some(subject) => clean_header(subject),
        None => match field(headers, "subject") {
            Some(subject) => format!("Auto: {}", clean_header(subject)),
            None => "Automated reply".to_owned(),
        },
    }
}

fn append_reply_references(output: &mut Vec<u8>, headers: &[(String, String)]) {
    let Some(message_id) = field(headers, "message-id") else {
        return;
    };
    let message_id = clean_header(message_id);
    if message_id.is_empty() {
        return;
    }
    write_header(output, "In-Reply-To", &message_id);
    let references = field(headers, "references")
        .map(clean_header)
        .unwrap_or_default();
    let updated = match references.is_empty() {
        true => message_id,
        false if references.split_ascii_whitespace().last() == Some(message_id.as_str()) => {
            references
        }
        false => format!("{references} {message_id}"),
    };
    write_header(output, "References", &updated);
}

fn append_utf8_text(output: &mut Vec<u8>, body: &[u8]) {
    output.extend_from_slice(b"MIME-Version: 1.0\r\nContent-Type: text/plain; charset=UTF-8\r\n");
    match body.iter().any(|byte| !byte.is_ascii()) {
        true => {
            output.extend_from_slice(b"Content-Transfer-Encoding: quoted-printable\r\n\r\n");
            output.extend_from_slice(&crate::mime::quoted_printable_encode::encode(body));
        }
        false => {
            output.extend_from_slice(b"Content-Transfer-Encoding: 7bit\r\n\r\n");
            output.extend_from_slice(body);
        }
    }
    ensure_crlf(output);
}

fn append_mime_entity(output: &mut Vec<u8>, entity: &[u8]) -> io::Result<()> {
    validate_mime_reason(entity).map_err(io::Error::other)?;
    let parsed = crate::mime::header_block::parse(entity).map_err(io::Error::other)?;
    let header_end = parsed
        .body_offset
        .checked_sub(2)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid MIME boundary"))?;
    let raw_headers = entity
        .get(..header_end)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid MIME headers"))?;
    match parsed
        .fields
        .iter()
        .any(|field| field.name == "mime-version")
    {
        true => {}
        false => output.extend_from_slice(b"MIME-Version: 1.0\r\n"),
    }
    output.extend_from_slice(raw_headers);
    output.extend_from_slice(b"\r\n");
    let body = entity
        .get(parsed.body_offset..)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid MIME body"))?;
    output.extend_from_slice(body);
    ensure_crlf(output);
    Ok(())
}

/// CHECKSCRIPT отвергает MIME entity, которую 7BIT-only transport не передаст.
pub(super) fn validate_mime_reason(entity: &[u8]) -> Result<(), &'static str> {
    let parsed = crate::mime::header_block::parse(entity)?;
    let header_end = parsed
        .body_offset
        .checked_sub(2)
        .ok_or("invalid MIME boundary")?;
    let raw_headers = entity.get(..header_end).ok_or("invalid MIME headers")?;
    if raw_headers.iter().any(|byte| !byte.is_ascii()) {
        return Err("8bit MIME header");
    }
    for field in &parsed.fields {
        if !mime_entity_header(&field.name) {
            return Err("non-MIME entity header");
        }
    }
    // MIME допускает ровно одно поле Content-Transfer-Encoding на сущность.
    // Сначала выбираем его как типизированный enum, затем тем же декодером,
    // который использует остальной сервер, проверяем фактические octets тела.
    let transfer_fields = parsed
        .fields
        .iter()
        .filter(|field| field.name == "content-transfer-encoding")
        .collect::<Vec<_>>();
    let transfer_field = match transfer_fields.as_slice() {
        [] => None,
        [field] => Some(*field),
        _ => return Err("duplicate MIME transfer encoding"),
    };
    let transfer = match transfer_field {
        Some(field) => crate::mime::transfer::TransferEncoding::parse(&field.value)
            .map_err(|_| "invalid MIME transfer encoding")?,
        None => crate::mime::transfer::TransferEncoding::SevenBit,
    };
    let body = entity
        .get(parsed.body_offset..)
        .ok_or("invalid MIME body")?;
    match transfer {
        crate::mime::transfer::TransferEncoding::SevenBit
        | crate::mime::transfer::TransferEncoding::Base64
        | crate::mime::transfer::TransferEncoding::QuotedPrintable => {
            transfer
                .decode(body)
                .map_err(|_| "invalid encoded MIME body")?;
        }
        crate::mime::transfer::TransferEncoding::EightBit
        | crate::mime::transfer::TransferEncoding::Binary
        | crate::mime::transfer::TransferEncoding::Unsupported(_) => {
            return Err("unsupported MIME transfer encoding");
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum MimeEntityHeader {
    Version,
    ContentType,
    TransferEncoding,
    ContentId,
    Disposition,
    Description,
    Language,
    Extension,
    Unsupported,
}

fn mime_entity_header(name: &str) -> bool {
    match mime_entity_field(name) {
        MimeEntityHeader::Version
        | MimeEntityHeader::ContentType
        | MimeEntityHeader::TransferEncoding
        | MimeEntityHeader::ContentId
        | MimeEntityHeader::Disposition
        | MimeEntityHeader::Description
        | MimeEntityHeader::Language
        | MimeEntityHeader::Extension => true,
        MimeEntityHeader::Unsupported => false,
    }
}

fn mime_entity_field(name: &str) -> MimeEntityHeader {
    match name.to_ascii_lowercase().as_str() {
        "mime-version" => MimeEntityHeader::Version,
        "content-type" => MimeEntityHeader::ContentType,
        "content-transfer-encoding" => MimeEntityHeader::TransferEncoding,
        "content-id" => MimeEntityHeader::ContentId,
        "content-disposition" => MimeEntityHeader::Disposition,
        "content-description" => MimeEntityHeader::Description,
        "content-language" => MimeEntityHeader::Language,
        value if value.starts_with("content-") => MimeEntityHeader::Extension,
        _ => MimeEntityHeader::Unsupported,
    }
}

fn encode_subject(value: &str) -> String {
    if value.is_ascii() {
        return value.to_owned();
    }
    let mut words = Vec::new();
    let mut encoded = String::new();
    for character in value.chars() {
        let token = q_token(character);
        if encoded.len().saturating_add(token.len()) > 60 {
            words.push(format!("=?UTF-8?Q?{encoded}?="));
            encoded.clear();
        }
        encoded.push_str(&token);
    }
    if !encoded.is_empty() {
        words.push(format!("=?UTF-8?Q?{encoded}?="));
    }
    words.join("\r\n ")
}

fn q_token(character: char) -> String {
    if character == ' ' {
        return "_".to_owned();
    }
    let mut token = String::new();
    for byte in character.to_string().bytes() {
        match byte {
            b'!'..=b'<' | b'>'..=b'~' if !matches!(byte, b'=' | b'?' | b'_') => {
                token.push(char::from(byte));
            }
            _ => token.push_str(&format!("={byte:02X}")),
        }
    }
    token
}

fn field<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find_map(
        |(candidate, value)| match candidate.eq_ignore_ascii_case(name) {
            true => Some(value.as_str()),
            false => None,
        },
    )
}

fn clean_header(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '\r' | '\n' | '\0' | '\u{7f}' => ' ',
            value if value.is_control() => ' ',
            value => value,
        })
        .collect::<String>()
        .split_ascii_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn write_header(output: &mut Vec<u8>, name: &str, value: &str) {
    output.extend_from_slice(name.as_bytes());
    output.extend_from_slice(b": ");
    output.extend_from_slice(clean_header(value).as_bytes());
    output.extend_from_slice(b"\r\n");
}

fn ensure_crlf(output: &mut Vec<u8>) {
    if !output.ends_with(b"\r\n") {
        output.extend_from_slice(b"\r\n");
    }
}
