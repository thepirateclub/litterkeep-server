//! RFC 9051 BODYSTRUCTURE extension-field ordering and serialization.

use super::super::content_metadata::ContentMetadata;
use super::super::disposition::Disposition;

/// Первый extension slot различается для multipart и scalar entity.
#[derive(Clone, Copy)]
pub(super) enum Anchor {
    MultipartParameters,
    BodyMd5,
}

/// Возвращает extension suffix, оставляя расширение отсутствующим при NIL-only.
pub(super) fn encode(
    anchor: Anchor,
    disposition: Option<&Disposition>,
    metadata: &ContentMetadata,
) -> Result<String, &'static str> {
    match (
        anchor,
        metadata.content_md5.is_some(),
        disposition.is_some(),
        metadata.languages.is_some(),
        metadata.location.is_some(),
    ) {
        (Anchor::BodyMd5, false, false, false, false)
        | (Anchor::MultipartParameters, _, false, false, false) => return Ok(String::new()),
        _ => {}
    }

    let mut output = match (anchor, metadata.content_md5.as_deref()) {
        (Anchor::BodyMd5, Some(digest)) => format!(" {}", quote_string(digest)?),
        (Anchor::BodyMd5, None) | (Anchor::MultipartParameters, _) => String::from(" NIL"),
    };
    match (
        disposition,
        metadata.languages.as_ref(),
        metadata.location.as_ref(),
    ) {
        (Some(value), _, _) => {
            output.push(' ');
            output.push_str(&super::super::disposition::encode_bodystructure(value)?);
        }
        (None, Some(_), _) | (None, None, Some(_)) => output.push_str(" NIL"),
        (None, None, None) => {}
    }
    match (metadata.languages.as_ref(), metadata.location.as_ref()) {
        (Some(languages), _) => {
            output.push(' ');
            output.push_str(&encode_languages(languages)?);
        }
        (None, Some(_)) => output.push_str(" NIL"),
        (None, None) => {}
    }
    if let Some(location) = metadata.location.as_ref() {
        output.push(' ');
        output.push('"');
        output.push_str(&quote(location)?);
        output.push('"');
    }
    Ok(output)
}

/// Кодирует одиночный язык строкой, список языков списком строк.
fn encode_languages(
    languages: &[super::super::language_tag::LanguageTag],
) -> Result<String, &'static str> {
    match languages {
        [language] => quote_string(language.as_str()),
        many => {
            let mut output = String::from("(");
            let mut separator = "";
            for language in many {
                output.push_str(separator);
                output.push_str(&quote_string(language.as_str())?);
                separator = " ";
            }
            output.push(')');
            Ok(output)
        }
    }
}

/// Создаёт quoted IMAP string без CR/LF инъекции.
pub(super) fn quote_string(value: &str) -> Result<String, &'static str> {
    Ok(format!("\"{}\"", quote(value)?))
}

/// Экранирует quoted-specials и отвергает запрещённые CR/LF octets.
fn quote(value: &str) -> Result<String, &'static str> {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\r' | '\n' | '\0' => return Err("control octet in BODYSTRUCTURE value"),
            '\\' | '"' => {
                output.push('\\');
                output.push(character);
            }
            _ => output.push(character),
        }
    }
    Ok(output)
}
