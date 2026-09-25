//! Выбор CTE-декодированного leaf part для IMAP4rev2 BINARY.

use super::{FetchData, FetchError, Partial};
use crate::mime::parameters::MediaType;
use crate::mime::tree::{self, Entity};
use std::borrow::Cow;

/// Декодирует выбранную MIME-часть и применяет диапазон к decoded octets.
pub(super) fn select(
    message: &[u8],
    path: &[usize],
    partial: Option<Partial>,
) -> Result<FetchData<'static>, FetchError> {
    let tree = tree::parse(message).map_err(|_| FetchError::InvalidSection)?;
    let entity = resolve(&tree, path)?;
    ensure_leaf(entity)?;
    let mut bytes = decode_part(entity, message)?;
    if is_textual(&entity.media_type) {
        bytes = normalize_crlf(&bytes);
    }
    let selected = partial_bytes(bytes, partial)?;
    let label = binary_label(path, partial);
    Ok(FetchData {
        label,
        bytes: Cow::Owned(selected),
    })
}

/// Возвращает octet count тех же decoded bytes, которые отдаёт BINARY.
pub(super) fn decoded_size(message: &[u8], path: &[usize]) -> Result<usize, FetchError> {
    let tree = tree::parse(message).map_err(|_| FetchError::InvalidSection)?;
    let entity = resolve(&tree, path)?;
    ensure_leaf(entity)?;
    let bytes = decode_part(entity, message)?;
    match is_textual(&entity.media_type) {
        true => Ok(normalize_crlf(&bytes).len()),
        false => Ok(bytes.len()),
    }
}

fn resolve<'a>(root: &'a Entity, path: &[usize]) -> Result<&'a Entity, FetchError> {
    let Some((number, remaining)) = path.split_first() else {
        return Err(FetchError::InvalidSection);
    };
    match is_multipart(&root.media_type) {
        true => {
            let index = number.checked_sub(1).ok_or(FetchError::InvalidSection)?;
            let child = root.children.get(index).ok_or(FetchError::InvalidSection)?;
            match remaining.is_empty() {
                true => Ok(child),
                false => resolve(child, remaining),
            }
        }
        false => match (*number == 1, remaining.is_empty()) {
            (false, _) => Err(FetchError::InvalidSection),
            (true, true) => Ok(root),
            (true, false) => match &root.media_type {
                MediaType::MessageRfc822 => {
                    let nested = root.children.first().ok_or(FetchError::InvalidSection)?;
                    resolve(nested, remaining)
                }
                _ => Err(FetchError::InvalidSection),
            },
        },
    }
}

fn ensure_leaf(entity: &Entity) -> Result<(), FetchError> {
    match (&entity.media_type, entity.children.is_empty()) {
        (MediaType::MultipartMixed, _)
        | (MediaType::MultipartAlternative, _)
        | (MediaType::MultipartOther(_), _) => Err(FetchError::InvalidSection),
        (MediaType::MultipartRelated, _) | (_, false) => Err(FetchError::InvalidSection),
        (MediaType::MessageRfc822, _) => Err(FetchError::InvalidSection),
        _ => Ok(()),
    }
}

fn decode_part(entity: &Entity, message: &[u8]) -> Result<Vec<u8>, FetchError> {
    match &entity.transfer_encoding {
        crate::mime::transfer::TransferEncoding::Unsupported(_) => {
            Err(FetchError::UnknownContentTransferEncoding)
        }
        _ => entity
            .decoded_body(message)
            .map_err(|_| FetchError::InvalidSection),
    }
}

fn partial_bytes(bytes: Vec<u8>, partial: Option<Partial>) -> Result<Vec<u8>, FetchError> {
    let Some(partial) = partial else {
        return Ok(bytes);
    };
    let remaining = bytes.get(partial.offset..).unwrap_or_default();
    let count = partial.count.min(remaining.len());
    Ok(remaining
        .get(..count)
        .ok_or(FetchError::InvalidSection)?
        .to_vec())
}

fn binary_label(path: &[usize], partial: Option<Partial>) -> String {
    let path_text = path
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(".");
    match partial {
        Some(range) => format!("BINARY[{path_text}]<{}>", range.offset),
        None => format!("BINARY[{path_text}]"),
    }
}

fn is_multipart(media_type: &MediaType) -> bool {
    matches!(
        media_type,
        MediaType::MultipartMixed
            | MediaType::MultipartAlternative
            | MediaType::MultipartRelated
            | MediaType::MultipartOther(_)
    )
}

fn is_textual(media_type: &MediaType) -> bool {
    matches!(
        media_type,
        MediaType::TextPlain | MediaType::TextHtml | MediaType::TextOther(_)
    )
}

fn normalize_crlf(bytes: &[u8]) -> Vec<u8> {
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut iterator = bytes.iter().copied().peekable();
    while let Some(byte) = iterator.next() {
        match (byte, iterator.peek().copied()) {
            (b'\r', Some(b'\n')) => {
                normalized.push(byte);
                if let Some(next) = iterator.next() {
                    normalized.push(next);
                }
            }
            (b'\n', _) => normalized.extend_from_slice(b"\r\n"),
            _ => normalized.push(byte),
        }
    }
    normalized
}

#[cfg(test)]
#[path = "binary_tests.rs"]
mod tests;
