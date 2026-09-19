//! Формирование базового IMAP BODYSTRUCTURE из собственного MIME-дерева.

use super::parameters::MediaType;
use super::tree::Entity;
use std::collections::BTreeMap;

/// Кодирует структуру, используя размеры исходных encoded body в октетах.
pub fn encode(entity: &Entity, message: &[u8]) -> Result<String, &'static str> {
    match entity.children.is_empty() {
        true => encode_leaf(entity, message),
        false => encode_multipart(entity, message),
    }
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
        MediaType::TextPlain | MediaType::TextHtml => Ok(format!("{base} {})", line_count(body))),
        _ => Ok(format!("{base})")),
    }
}

fn encode_multipart(entity: &Entity, message: &[u8]) -> Result<String, &'static str> {
    let mut output = String::from("(");
    for child in &entity.children {
        output.push_str(&encode(child, message)?);
    }
    let (_, subtype) = entity.media_type.type_and_subtype();
    output.push_str(&format!(" \"{}\")", escaped_atom(subtype)?));
    Ok(output)
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
}
