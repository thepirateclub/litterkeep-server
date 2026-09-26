//! Извлечение MIME part, message headers/text и выбранных header fields.

use crate::imap::BodySection;
use crate::mime::header_block;
use crate::mime::parameters::MediaType;
use crate::mime::tree::{self, Entity};
use std::borrow::Cow;

/// Выбранная section до применения диапазона partial.
pub(super) struct SelectedSection<'a> {
    pub label: String,
    pub bytes: Cow<'a, [u8]>,
}

/// Сопоставляет каждую typed section с её исходными байтовыми границами.
pub(super) fn select<'a>(
    message: &'a [u8],
    section: &BodySection,
) -> Result<SelectedSection<'a>, &'static str> {
    match section {
        BodySection::EntireMessage => Ok(selected("BODY[]", Cow::Borrowed(message))),
        BodySection::Part { path, mime_headers } => select_part(message, path, *mime_headers),
        BodySection::Headers { path } => select_message_headers(message, path),
        BodySection::Text { path } => select_message_text(message, path),
        BodySection::HeaderFields {
            path,
            names,
            excluded,
        } => select_header_fields(message, path, names, *excluded),
    }
}

fn select_part<'a>(
    message: &'a [u8],
    path: &[usize],
    mime_headers: bool,
) -> Result<SelectedSection<'a>, &'static str> {
    let tree = tree::parse(message)?;
    let entity = part_at(&tree, path)?;
    let range = match mime_headers {
        true => entity.header_range.clone(),
        false => entity.body_range.clone(),
    };
    let suffix = match mime_headers {
        true => ".MIME",
        false => "",
    };
    let numbers = path
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(".");
    let bytes = match entity.uses_decoded_source() {
        true => Cow::Owned(
            entity
                .source_range(message, range)
                .ok_or("BODY section range is invalid")?
                .to_vec(),
        ),
        false => Cow::Borrowed(message.get(range).ok_or("BODY section range is invalid")?),
    };
    Ok(selected(&format!("BODY[{numbers}{suffix}]"), bytes))
}

fn select_message_headers<'a>(
    message: &'a [u8],
    path: &[usize],
) -> Result<SelectedSection<'a>, &'static str> {
    let (raw, label) = message_entity(message, path, "HEADER")?;
    let block = header_block::parse(&raw)?;
    let bytes = raw
        .get(..block.body_offset)
        .ok_or("HEADER section range is invalid")?;
    Ok(selected(&label, Cow::Owned(bytes.to_vec())))
}

fn select_message_text<'a>(
    message: &'a [u8],
    path: &[usize],
) -> Result<SelectedSection<'a>, &'static str> {
    let (raw, label) = message_entity(message, path, "TEXT")?;
    let block = header_block::parse(&raw)?;
    let bytes = raw
        .get(block.body_offset..)
        .ok_or("TEXT section range is invalid")?;
    Ok(selected(&label, Cow::Owned(bytes.to_vec())))
}

fn select_header_fields<'a>(
    message: &'a [u8],
    path: &[usize],
    names: &[String],
    excluded: bool,
) -> Result<SelectedSection<'a>, &'static str> {
    let selector = match excluded {
        true => "HEADER.FIELDS.NOT",
        false => "HEADER.FIELDS",
    };
    let (raw, label) = message_entity(message, path, selector)?;
    let block = header_block::parse(&raw)?;
    let mut bytes = Vec::new();
    for field in &block.fields {
        let matched = names
            .iter()
            .any(|name| field.name.eq_ignore_ascii_case(name));
        if matched != excluded {
            let field_bytes = raw
                .get(field.raw_range.clone())
                .ok_or("header field range is invalid")?;
            bytes.extend_from_slice(field_bytes);
        }
    }
    bytes.extend_from_slice(b"\r\n");
    let field_list = names.join(" ");
    let label = format!("{} ({field_list})]", label.trim_end_matches(']'));
    Ok(selected(&label, Cow::Owned(bytes)))
}

fn message_entity(
    message: &[u8],
    path: &[usize],
    selector: &str,
) -> Result<(Vec<u8>, String), &'static str> {
    let tree = tree::parse(message)?;
    let raw = match path.is_empty() {
        true => message.to_vec(),
        false => {
            let part = part_at(&tree, path)?;
            match is_message(&part.media_type) {
                true => part
                    .children
                    .first()
                    .ok_or("encapsulated message is absent")?
                    .raw_bytes(message)?
                    .to_vec(),
                false => return Err("HEADER/TEXT selectors require an encapsulated message"),
            }
        }
    };
    let path_label = path
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(".");
    let label = match path_label.is_empty() {
        true => format!("BODY[{selector}]"),
        false => format!("BODY[{path_label}.{selector}]"),
    };
    Ok((raw, label))
}

/// Преобразует IMAP part-number путь с отдельным уровнем для embedded message.
fn part_at<'a>(root: &'a Entity, path: &[usize]) -> Result<&'a Entity, &'static str> {
    let (first, remaining) = path.split_first().ok_or("BODY part number is absent")?;
    let mut current = numbered_part(root, *first)?;
    for number in remaining {
        current = match &current.media_type {
            MediaType::MultipartMixed
            | MediaType::MultipartAlternative
            | MediaType::MultipartRelated
            | MediaType::MultipartDigest
            | MediaType::MultipartOther(_) => numbered_part(current, *number)?,
            MediaType::MessageRfc822 | MediaType::MessageGlobal => {
                let embedded = current
                    .children
                    .first()
                    .ok_or("encapsulated message is absent")?;
                numbered_part(embedded, *number)?
            }
            _ => return Err("BODY part has no nested parts"),
        };
    }
    Ok(current)
}

/// Возвращает номер части multipart либо единственную часть номер один.
fn numbered_part(entity: &Entity, number: usize) -> Result<&Entity, &'static str> {
    match &entity.media_type {
        MediaType::MultipartMixed
        | MediaType::MultipartAlternative
        | MediaType::MultipartRelated
        | MediaType::MultipartDigest
        | MediaType::MultipartOther(_) => entity
            .children
            .get(
                number
                    .checked_sub(1)
                    .ok_or("BODY part number starts at one")?,
            )
            .ok_or("BODY part does not exist"),
        _ => match number {
            1 => Ok(entity),
            _ => Err("single-part MIME entity only has part one"),
        },
    }
}

/// Проверяет тип embedded message через MIME enum, не wire-строку.
fn is_message(media_type: &MediaType) -> bool {
    matches!(
        media_type,
        MediaType::MessageRfc822 | MediaType::MessageGlobal
    )
}

fn selected<'a>(label: &str, bytes: Cow<'a, [u8]>) -> SelectedSection<'a> {
    SelectedSection {
        label: label.to_owned(),
        bytes,
    }
}
