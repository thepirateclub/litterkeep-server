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
    mode: crate::imap::tokenizer::StringMode,
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
        (Anchor::BodyMd5, Some(digest)) => format!(" {}", quote_string(digest, mode)?),
        (Anchor::BodyMd5, None) | (Anchor::MultipartParameters, _) => String::from(" NIL"),
    };
    match (
        disposition,
        metadata.languages.as_ref(),
        metadata.location.as_ref(),
    ) {
        (Some(value), _, _) => {
            output.push(' ');
            output.push_str(&super::super::disposition::encode_bodystructure_with_mode(
                value, mode,
            )?);
        }
        (None, Some(_), _) | (None, None, Some(_)) => output.push_str(" NIL"),
        (None, None, None) => {}
    }
    match (metadata.languages.as_ref(), metadata.location.as_ref()) {
        (Some(languages), _) => {
            output.push(' ');
            output.push_str(&encode_languages(languages, mode)?);
        }
        (None, Some(_)) => output.push_str(" NIL"),
        (None, None) => {}
    }
    if let Some(location) = metadata.location.as_ref() {
        output.push(' ');
        output.push_str(&quote_string(location, mode)?);
    }
    Ok(output)
}

/// Кодирует одиночный язык строкой, список языков списком строк.
fn encode_languages(
    languages: &[super::super::language_tag::LanguageTag],
    mode: crate::imap::tokenizer::StringMode,
) -> Result<String, &'static str> {
    match languages {
        [language] => quote_string(language.as_str(), mode),
        many => {
            let mut output = String::from("(");
            let mut separator = "";
            for language in many {
                output.push_str(separator);
                output.push_str(&quote_string(language.as_str(), mode)?);
                separator = " ";
            }
            output.push(')');
            Ok(output)
        }
    }
}

/// Создаёт quoted IMAP string без CR/LF инъекции.
pub(super) fn quote_string(
    value: &str,
    mode: crate::imap::tokenizer::StringMode,
) -> Result<String, &'static str> {
    let escaped = quote(value)?;
    match (mode, value.is_ascii()) {
        (crate::imap::tokenizer::StringMode::Legacy, false) => {
            Ok(format!("{{{}}}\r\n{value}", value.len()))
        }
        _ => Ok(format!("\"{escaped}\"")),
    }
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
