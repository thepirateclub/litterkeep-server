//! Извлечение MIME part, message headers/text и выбранных header fields.

use crate::imap::BodySection;
use crate::mime::header_block;
use crate::mime::parameters::MediaType;
use crate::mime::tree::{self, Entity};
use std::borrow::Cow;
use std::ops::Range;

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
    let entity = entity_at(&tree, path)?;
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
    let bytes = message.get(range).ok_or("BODY section range is invalid")?;
    Ok(selected(
        &format!("BODY[{numbers}{suffix}]"),
        Cow::Borrowed(bytes),
    ))
}

fn select_message_headers<'a>(
    message: &'a [u8],
    path: &[usize],
) -> Result<SelectedSection<'a>, &'static str> {
    let (entity_range, label) = message_entity(message, path, "HEADER")?;
    let raw = raw_entity(message, entity_range)?;
    let block = header_block::parse(raw)?;
    let bytes = raw
        .get(..block.body_offset)
        .ok_or("HEADER section range is invalid")?;
    Ok(selected(&label, Cow::Borrowed(bytes)))
}

fn select_message_text<'a>(
    message: &'a [u8],
    path: &[usize],
) -> Result<SelectedSection<'a>, &'static str> {
    let (entity_range, label) = message_entity(message, path, "TEXT")?;
    let raw = raw_entity(message, entity_range)?;
    let block = header_block::parse(raw)?;
    let bytes = raw
        .get(block.body_offset..)
        .ok_or("TEXT section range is invalid")?;
    Ok(selected(&label, Cow::Borrowed(bytes)))
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
    let (entity_range, label) = message_entity(message, path, selector)?;
    let raw = raw_entity(message, entity_range)?;
    let block = header_block::parse(raw)?;
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
) -> Result<(Range<usize>, String), &'static str> {
    let tree = tree::parse(message)?;
    let entity = entity_at(&tree, path)?;
    validate_message_path(&tree, path)?;
    let path_label = path
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(".");
    let label = match path_label.is_empty() {
        true => format!("BODY[{selector}]"),
        false => format!("BODY[{path_label}.{selector}]"),
    };
    Ok((entity.raw_range.clone(), label))
}

fn validate_message_path(root: &Entity, path: &[usize]) -> Result<(), &'static str> {
    let Some((_last, parent_path)) = path.split_last() else {
        return Ok(());
    };
    let parent = entity_at(root, parent_path)?;
    match &parent.media_type {
        MediaType::MessageRfc822 => Ok(()),
        _ => Err("HEADER/TEXT selectors require an encapsulated message"),
    }
}

fn raw_entity(message: &[u8], range: Range<usize>) -> Result<&[u8], &'static str> {
    message.get(range).ok_or("MIME entity range is invalid")
}

fn entity_at<'a>(root: &'a Entity, path: &[usize]) -> Result<&'a Entity, &'static str> {
    let mut current = root;
    for number in path {
        let index = number
            .checked_sub(1)
            .ok_or("BODY part number starts at one")?;
        current = current
            .children
            .get(index)
            .ok_or("BODY part does not exist")?;
    }
    Ok(current)
}

fn selected<'a>(label: &str, bytes: Cow<'a, [u8]>) -> SelectedSection<'a> {
    SelectedSection {
        label: label.to_owned(),
        bytes,
    }
}
