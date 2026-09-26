//! Формирование базового IMAP BODYSTRUCTURE из собственного MIME-дерева.

#[path = "bodystructure_extensions.rs"]
mod extensions;
#[cfg(test)]
#[path = "bodystructure_profile_tests.rs"]
mod profile_tests;
#[cfg(test)]
#[path = "bodystructure_tests.rs"]
mod tests;

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
        MediaType::MessageRfc822 | MediaType::MessageGlobal => {
            encode_message(entity, message, profile)
        }
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
        "(\"MESSAGE\" \"{}\" {} {} {} \"{}\" {} {envelope} {nested_structure} {}{})",
        entity.media_type.type_and_subtype().1,
        encode_parameters(&entity.parameters)?,
        encode_optional_string(entity.content_metadata.content_id.as_deref())?,
        encode_optional_string(entity.content_metadata.description.as_deref())?,
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
        "(\"{}\" \"{}\" {parameters} {} {} \"{}\" {}",
        escaped_atom(media_type)?,
        escaped_atom(subtype)?,
        encode_optional_string(entity.content_metadata.content_id.as_deref())?,
        encode_optional_string(entity.content_metadata.description.as_deref())?,
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

/// Кодирует MIME basic nstring или NIL для отсутствующего поля.
fn encode_optional_string(value: Option<&str>) -> Result<String, &'static str> {
    match value {
        Some(text) => extensions::quote_string(text),
        None => Ok("NIL".to_owned()),
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
