//! Формирование базового IMAP BODYSTRUCTURE из собственного MIME-дерева.

use super::parameters::MediaType;
use super::tree::Entity;
use std::collections::BTreeMap;

/// Кодирует структуру, используя размеры исходных encoded body в октетах.
pub fn encode(entity: &Entity, message: &[u8]) -> Result<String, &'static str> {
    match &entity.media_type {
        MediaType::MessageRfc822 => encode_message(entity, message),
        _ => match entity.children.is_empty() {
            true => encode_leaf(entity, message),
            false => encode_multipart(entity, message),
        },
    }
}

fn encode_message(entity: &Entity, message: &[u8]) -> Result<String, &'static str> {
    let body = message
        .get(entity.body_range.clone())
        .ok_or("message/rfc822 range is outside message")?;
    let nested = entity
        .children
        .first()
        .ok_or("message/rfc822 child is absent")?;
    let envelope = super::envelope::encode(body)?;
    let nested_structure = encode(nested, message)?;
    Ok(format!(
        "(\"MESSAGE\" \"RFC822\" {} NIL NIL \"{}\" {} {envelope} {nested_structure} {}{})",
        encode_parameters(&entity.parameters)?,
        escaped_atom(entity.transfer_encoding.wire_name())?,
        body.len(),
        line_count(body),
        encode_disposition_extension(entity)?
    ))
}

fn encode_leaf(entity: &Entity, message: &[u8]) -> Result<String, &'static str> {
    let body = message
        .get(entity.body_range.clone())
        .ok_or("BODYSTRUCTURE range is outside message")?;
    let (media_type, subtype) = entity.media_type.type_and_subtype();
    let parameters = encode_parameters(&entity.parameters)?;
    let base = format!(
        "(\"{}\" \"{}\" {parameters} NIL NIL \"{}\" {}",
        escaped_atom(media_type)?,
        escaped_atom(subtype)?,
        escaped_atom(entity.transfer_encoding.wire_name())?,
        body.len()
    );
    match entity.media_type {
        MediaType::TextPlain | MediaType::TextHtml | MediaType::TextOther(_) => Ok(format!(
            "{base} {}{})",
            line_count(body),
            encode_disposition_extension(entity)?
        )),
        _ => Ok(format!("{base}{})", encode_disposition_extension(entity)?)),
    }
}

fn encode_multipart(entity: &Entity, message: &[u8]) -> Result<String, &'static str> {
    let mut output = String::from("(");
    for child in &entity.children {
        output.push_str(&encode(child, message)?);
    }
    let (_, subtype) = entity.media_type.type_and_subtype();
    output.push_str(&format!(
        " \"{}\"{})",
        escaped_atom(subtype)?,
        encode_disposition_extension(entity)?
    ));
    Ok(output)
}

/// Размещает disposition после обязательного NIL body-MD5 extension slot.
fn encode_disposition_extension(entity: &Entity) -> Result<String, &'static str> {
    match &entity.disposition {
        Some(value) => Ok(format!(
            " NIL {}",
            super::disposition::encode_bodystructure(value)?
        )),
        None => Ok(String::new()),
    }
}

fn encode_parameters(parameters: &BTreeMap<String, String>) -> Result<String, &'static str> {
    if parameters.is_empty() {
        return Ok("NIL".to_owned());
    }
    let mut output = String::from("(");
    let mut separator = "";
    for (name, value) in parameters {
        output.push_str(separator);
        output.push_str(&format!(
            "\"{}\" \"{}\"",
            escaped_atom(&name.to_ascii_uppercase())?,
            escaped_atom(value)?
        ));
        separator = " ";
    }
    output.push(')');
    Ok(output)
}

fn escaped_atom(value: &str) -> Result<String, &'static str> {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\r' | '\n' => return Err("newline in BODYSTRUCTURE value"),
            '\\' | '"' => {
                output.push('\\');
                output.push(character);
            }
            _ => output.push(character),
        }
    }
    Ok(output)
}

fn line_count(body: &[u8]) -> usize {
    body.windows(2).filter(|pair| *pair == b"\r\n").count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_encoded_octets_and_text_lines() {
        let raw = b"Content-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\naGVsbG8NCg==";
        let entity = super::super::tree::parse(raw).expect("entity");
        assert_eq!(
            encode(&entity, raw).expect("bodystructure"),
            "(\"TEXT\" \"PLAIN\" (\"CHARSET\" \"utf-8\") NIL NIL \"BASE64\" 12 0)"
        );
    }

    #[test]
    fn encodes_multipart_children_in_order() {
        let raw = b"Content-Type: multipart/alternative; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\na\r\n--x\r\nContent-Type: text/html\r\n\r\n<b>a</b>\r\n--x--\r\n";
        let entity = super::super::tree::parse(raw).expect("entity");
        let encoded = encode(&entity, raw).expect("bodystructure");
        assert!(encoded.ends_with(" \"ALTERNATIVE\")"));
        assert!(encoded.contains("\"TEXT\" \"PLAIN\""));
        assert!(encoded.contains("\"TEXT\" \"HTML\""));
    }

    #[test]
    fn encodes_nested_message_envelope_and_structure() {
        let raw = b"Content-Type: message/rfc822\r\n\r\nFrom: a@test\r\nTo: b@test\r\nSubject: nested\r\nContent-Type: text/plain\r\n\r\nbody";
        let entity = super::super::tree::parse(raw).expect("entity");
        let encoded = encode(&entity, raw).expect("bodystructure");
        assert!(encoded.starts_with("(\"MESSAGE\" \"RFC822\""));
        assert!(encoded.contains("\"nested\""));
        assert!(encoded.contains("\"TEXT\" \"PLAIN\""));
    }

    #[test]
    fn encodes_headerless_multipart_digest_as_nested_message() {
        let raw = b"Content-Type: multipart/digest; boundary=x\r\n\r\n--x\r\n\r\nSubject: nested\r\n\r\nmessage body\r\n--x--\r\n";
        let entity = super::super::tree::parse(raw).expect("multipart digest");

        assert_eq!(
            encode(&entity, raw).expect("BODYSTRUCTURE"),
            "((\"MESSAGE\" \"RFC822\" NIL NIL NIL \"7BIT\" 31 (NIL \"nested\" NIL NIL NIL NIL NIL NIL NIL NIL) (\"TEXT\" \"PLAIN\" NIL NIL NIL \"7BIT\" 12 0) 2) \"DIGEST\")"
        );
    }

    #[test]
    fn unknown_transfer_encoding_reports_effective_octet_stream_type() {
        let raw = b"Content-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: x-private\r\n\r\nopaque";
        let entity = super::super::tree::parse(raw).expect("opaque entity");

        assert_eq!(
            encode(&entity, raw).expect("BODYSTRUCTURE"),
            "(\"APPLICATION\" \"OCTET-STREAM\" NIL NIL NIL \"x-private\" 6)"
        );
    }

    #[test]
    fn bodystructure_includes_rfc2183_disposition_after_body_md5_slot() {
        let raw = b"Content-Type: application/pdf\r\nContent-Disposition: attachment; filename*=UTF-8''report%20Q1.pdf\r\n\r\npdf";
        let entity = super::super::tree::parse(raw).expect("MIME entity");

        assert_eq!(
            encode(&entity, raw).expect("BODYSTRUCTURE"),
            "(\"application\" \"pdf\" NIL NIL NIL \"7BIT\" 3 NIL (\"attachment\" (\"FILENAME\" \"report Q1.pdf\")))"
        );
    }

    #[test]
    fn multipart_disposition_follows_subtype_extension_position() {
        let raw = b"Content-Type: multipart/mixed; boundary=x\r\nContent-Disposition: inline\r\n\r\n--x\r\n\r\nbody\r\n--x--\r\n";
        let entity = super::super::tree::parse(raw).expect("multipart entity");

        assert_eq!(
            encode(&entity, raw).expect("BODYSTRUCTURE"),
            "((\"TEXT\" \"PLAIN\" NIL NIL NIL \"7BIT\" 4 0) \"MIXED\" NIL (\"inline\" NIL))"
        );
    }

    #[test]
    fn message_rfc822_disposition_follows_embedded_message_fields() {
        let raw = b"Content-Type: message/rfc822\r\nContent-Disposition: attachment; filename=forwarded.eml\r\n\r\nSubject: nested\r\n\r\nbody";
        let entity = super::super::tree::parse(raw).expect("message entity");

        assert!(
            encode(&entity, raw)
                .expect("BODYSTRUCTURE")
                .ends_with(" 2 NIL (\"attachment\" (\"FILENAME\" \"forwarded.eml\")))")
        );
    }
}
