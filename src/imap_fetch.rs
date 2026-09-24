//! Выбор RFC IMAP BODY sections и применение октетного partial range.

use crate::imap::{FetchAttribute, Partial};
use std::borrow::Cow;

#[cfg(any(target_os = "linux", test))]
#[path = "imap_fetch/binary.rs"]
mod binary;
#[path = "imap_fetch/sections.rs"]
mod sections;

/// Literal-данные и имя data item для ответа FETCH.
pub struct FetchData<'a> {
    pub label: String,
    pub bytes: Cow<'a, [u8]>,
}

/// Результат FETCH section parsing без потери UNKNOWN-CTE на wire-границе.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FetchError {
    InvalidSection,
    UnknownContentTransferEncoding,
}

/// Извлекает исходную BODY section, затем применяет partial к уже выбранным октетам.
pub fn select<'a>(
    message: &'a [u8],
    attribute: &FetchAttribute,
) -> Result<FetchData<'a>, &'static str> {
    let FetchAttribute::Body {
        section, partial, ..
    } = attribute
    else {
        return Err("attribute is not BODY");
    };
    let selected = sections::select(message, section)?;
    apply_partial(selected.label, selected.bytes, *partial)
}

/// Декодирует один typed BINARY item, не меняя владельца оригинального письма.
#[cfg(target_os = "linux")]
pub(crate) fn select_binary(
    message: &[u8],
    attribute: &FetchAttribute,
) -> Result<FetchData<'static>, FetchError> {
    let FetchAttribute::Binary {
        section, partial, ..
    } = attribute
    else {
        return Err(FetchError::InvalidSection);
    };
    binary::select(message, section, *partial)
}

/// Вычисляет decoded SIZE для одного typed BINARY.SIZE item.
#[cfg(target_os = "linux")]
pub(crate) fn binary_size(message: &[u8], attribute: &FetchAttribute) -> Result<usize, FetchError> {
    let FetchAttribute::BinarySize { section } = attribute else {
        return Err(FetchError::InvalidSection);
    };
    binary::decoded_size(message, section)
}

fn apply_partial<'a>(
    label: String,
    bytes: Cow<'a, [u8]>,
    partial: Option<Partial>,
) -> Result<FetchData<'a>, &'static str> {
    let Some(partial) = partial else {
        return Ok(FetchData { label, bytes });
    };
    let available = bytes.get(partial.offset..).unwrap_or_default();
    let count = partial.count.min(available.len());
    let selected = available.get(..count).ok_or("invalid BODY partial range")?;
    Ok(FetchData {
        label: format!("{label}<{}>", partial.offset),
        bytes: Cow::Owned(selected.to_vec()),
    })
}

#[cfg(test)]
#[path = "imap_fetch/tests.rs"]
mod tests;
