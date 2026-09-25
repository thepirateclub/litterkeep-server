//! Формирование базового IMAP BODYSTRUCTURE из собственного MIME-дерева.

#[path = "bodystructure_extensions.rs"]
mod extensions;
#[cfg(test)]
#[path = "bodystructure_profile_tests.rs"]
mod profile_tests;

use super::parameters::MediaType;
use super::tree::Entity;
use std::collections::BTreeMap;

/// Кодирует структуру, используя размеры исходных encoded body в октетах.
pub fn encode(entity: &Entity, message: &[u8]) -> Result<String, &'static str> {
    encode_basic(entity, message)
}

/// Кодирует BODY без расширенных параметров IMAP.
pub fn encode_basic(entity: &Entity, message: &[u8]) -> Result<String, &'static str> {
    encode_in_profile(entity, message, Profile::Basic)
}

/// Кодирует BODYSTRUCTURE с RFC 9051 extension fields.
pub fn encode_extended(entity: &Entity, message: &[u8]) -> Result<String, &'static str> {
    encode_in_profile(entity, message, Profile::Extended)
}

/// Различает wire-форму BODY от BODYSTRUCTURE без флага-предиката.
#[derive(Clone, Copy)]
enum Profile {
    Basic,
    Extended,
}

fn encode_in_profile(
    entity: &Entity,
    message: &[u8],
    profile: Profile,
) -> Result<String, &'static str> {
    match &entity.media_type {
        MediaType::MessageRfc822 => encode_message(entity, message, profile),
        _ => match entity.children.is_empty() {
            true => encode_leaf(entity, message, profile),
            false => encode_multipart(entity, message, profile),
        },
    }
}

fn encode_message(
    entity: &Entity,
    message: &[u8],
    profile: Profile,
) -> Result<String, &'static str> {
    let body = message
        .get(entity.body_range.clone())
        .ok_or("message/rfc822 range is outside message")?;
    let nested = entity
        .children
        .first()
        .ok_or("message/rfc822 child is absent")?;
    let envelope = super::envelope::encode(body)?;
    let nested_structure = encode_in_profile(nested, message, profile)?;
    Ok(format!(
        "(\"MESSAGE\" \"RFC822\" {} NIL NIL \"{}\" {} {envelope} {nested_structure} {}{})",
        encode_parameters(&entity.parameters)?,
        escaped_atom(entity.transfer_encoding.wire_name())?,
        body.len(),
        line_count(body),
        encode_extensions(entity, extensions::Anchor::BodyMd5, profile)?
    ))
}

fn encode_leaf(entity: &Entity, message: &[u8], profile: Profile) -> Result<String, &'static str> {
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
            encode_extensions(entity, extensions::Anchor::BodyMd5, profile)?
        )),
        _ => Ok(format!(
            "{base}{})",
            encode_extensions(entity, extensions::Anchor::BodyMd5, profile)?
        )),
    }
}

fn encode_multipart(
    entity: &Entity,
    message: &[u8],
    profile: Profile,
) -> Result<String, &'static str> {
    let mut output = String::from("(");
    for child in &entity.children {
        output.push_str(&encode_in_profile(child, message, profile)?);
    }
    let (_, subtype) = entity.media_type.type_and_subtype();
    output.push_str(&format!(
        " \"{}\"{})",
        escaped_atom(subtype)?,
        encode_extensions(entity, extensions::Anchor::MultipartParameters, profile)?
    ));
    Ok(output)
}

/// Добавляет extension suffix только в BODYSTRUCTURE, не в BODY.
fn encode_extensions(
    entity: &Entity,
    anchor: extensions::Anchor,
    profile: Profile,
) -> Result<String, &'static str> {
    match profile {
        Profile::Basic => Ok(String::new()),
        Profile::Extended => extensions::encode(
            anchor,
            entity.disposition.as_ref(),
            &entity.content_metadata,
        ),
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
            encode_extended(&entity, raw).expect("BODYSTRUCTURE"),
            "((\"MESSAGE\" \"RFC822\" NIL NIL NIL \"7BIT\" 31 (NIL \"nested\" NIL NIL NIL NIL NIL NIL NIL NIL) (\"TEXT\" \"PLAIN\" NIL NIL NIL \"7BIT\" 12 0) 2) \"DIGEST\")"
        );
    }

    #[test]
    fn unknown_transfer_encoding_reports_effective_octet_stream_type() {
        let raw = b"Content-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: x-private\r\n\r\nopaque";
        let entity = super::super::tree::parse(raw).expect("opaque entity");

        assert_eq!(
            encode_extended(&entity, raw).expect("BODYSTRUCTURE"),
            "(\"APPLICATION\" \"OCTET-STREAM\" NIL NIL NIL \"x-private\" 6)"
        );
    }

    #[test]
    fn bodystructure_includes_rfc2183_disposition_after_body_md5_slot() {
        let raw = b"Content-Type: application/pdf\r\nContent-Disposition: attachment; filename*=UTF-8''report%20Q1.pdf\r\n\r\npdf";
        let entity = super::super::tree::parse(raw).expect("MIME entity");

        assert_eq!(
            encode_extended(&entity, raw).expect("BODYSTRUCTURE"),
            "(\"application\" \"pdf\" NIL NIL NIL \"7BIT\" 3 NIL (\"attachment\" (\"FILENAME\" \"report Q1.pdf\")))"
        );
    }

    #[test]
    fn multipart_disposition_follows_subtype_extension_position() {
        let raw = b"Content-Type: multipart/mixed; boundary=x\r\nContent-Disposition: inline\r\n\r\n--x\r\n\r\nbody\r\n--x--\r\n";
        let entity = super::super::tree::parse(raw).expect("multipart entity");

        assert_eq!(
            encode_extended(&entity, raw).expect("BODYSTRUCTURE"),
            "((\"TEXT\" \"PLAIN\" NIL NIL NIL \"7BIT\" 4 0) \"MIXED\" NIL (\"inline\" NIL))"
        );
    }

    #[test]
    fn message_rfc822_disposition_follows_embedded_message_fields() {
        let raw = b"Content-Type: message/rfc822\r\nContent-Disposition: attachment; filename=forwarded.eml\r\n\r\nSubject: nested\r\n\r\nbody";
        let entity = super::super::tree::parse(raw).expect("message entity");

        assert!(
            encode_extended(&entity, raw)
                .expect("BODYSTRUCTURE")
                .ends_with(" 2 NIL (\"attachment\" (\"FILENAME\" \"forwarded.eml\")))")
        );
    }

    #[test]
    fn language_and_location_extensions_follow_disposition_in_rfc_order() {
        let raw = b"Content-Type: application/pdf\r\nContent-Disposition: attachment\r\nContent-Language: en-US, fr\r\nContent-Location: ../files/report%20one.pdf\r\n\r\npdf";
        let entity = super::super::tree::parse(raw).expect("MIME entity");

        assert_eq!(
            encode_extended(&entity, raw).expect("BODYSTRUCTURE"),
            "(\"application\" \"pdf\" NIL NIL NIL \"7BIT\" 3 NIL (\"attachment\" NIL) (\"en-US\" \"fr\") \"../files/report%20one.pdf\")"
        );
    }

    #[test]
    fn location_only_fills_disposition_and_language_slots_with_nil() {
        let raw =
            b"Content-Type: application/octet-stream\r\nContent-Location: ./part.bin\r\n\r\nx";
        let entity = super::super::tree::parse(raw).expect("MIME entity");

        assert_eq!(
            encode_extended(&entity, raw).expect("BODYSTRUCTURE"),
            "(\"APPLICATION\" \"OCTET-STREAM\" NIL NIL NIL \"7BIT\" 1 NIL NIL NIL \"./part.bin\")"
        );
    }

    #[test]
    fn multipart_language_and_location_use_parameters_as_first_extension() {
        let raw = b"Content-Type: multipart/mixed; boundary=x\r\nContent-Language: en\r\nContent-Location: https://example.test/mail/\r\n\r\n--x\r\n\r\nbody\r\n--x--\r\n";
        let entity = super::super::tree::parse(raw).expect("multipart entity");

        assert!(
            encode_extended(&entity, raw)
                .expect("BODYSTRUCTURE")
                .ends_with(" \"MIXED\" NIL NIL \"en\" \"https://example.test/mail/\")")
        );
    }
}
