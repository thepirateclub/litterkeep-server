//! Выбор raw IMAP BODY section и применение октетного partial range.

use crate::imap::{BodySection, FetchAttribute, Partial};
use crate::mime::tree::{self, Entity};

/// Данные literal и каноническая метка ответа FETCH.
pub struct FetchData<'a> {
    pub label: String,
    pub bytes: &'a [u8],
}

/// Выбирает section без transfer decoding и повторной сериализации MIME.
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
    let tree = tree::parse(message)?;
    let (label, bytes) = section_bytes(message, &tree, section)?;
    apply_partial(label, bytes, *partial)
}

fn section_bytes<'a>(
    message: &'a [u8],
    tree: &Entity,
    section: &BodySection,
) -> Result<(String, &'a [u8]), &'static str> {
    match section {
        BodySection::EntireMessage => Ok(("BODY[]".to_owned(), message)),
        BodySection::Part { path, mime_headers } => {
            let entity = entity_at(tree, path)?;
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
            let bytes = message
                .get(range)
                .ok_or("BODY section range is outside message")?;
            Ok((format!("BODY[{numbers}{suffix}]"), bytes))
        }
    }
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

fn apply_partial<'a>(
    label: String,
    bytes: &'a [u8],
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
        bytes: selected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_encoded_part_and_octet_partial() {
        let message = b"Content-Type: multipart/mixed; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\nabcdef\r\n--x--\r\n";
        let attribute = FetchAttribute::Body {
            peek: true,
            section: BodySection::Part {
                path: vec![1],
                mime_headers: false,
            },
            partial: Some(Partial {
                offset: 2,
                count: 3,
            }),
        };
        let selected = select(message, &attribute).expect("section");
        assert_eq!(selected.label, "BODY[1]<2>");
        assert_eq!(selected.bytes, b"cde");
    }
}
