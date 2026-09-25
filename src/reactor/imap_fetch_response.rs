//! Кодирование общего untagged FETCH response для одного сообщения.
//!
//! Несколько запрошенных data item сериализуются в один список, как требует
//! grammar IMAP, а literal остаётся в потоке необработанными октетами. Для
//! UID FETCH номер UID добавляется независимо от списка клиента и повторный
//! UID item подавляется. Размер временного ответа ограничивается тем же
//! пределом, что и выходной буфер сетевой сессии.

use super::*;

pub(super) fn write(
    session: &mut Session,
    number: usize,
    message: &crate::store::Message,
    attributes: &[imap::FetchAttribute],
    addressing: imap::Addressing,
) -> io::Result<Result<(), crate::imap_fetch::FetchError>> {
    let mut response = format!("* {number} FETCH (").into_bytes();
    let mut first = true;
    let mut uid_written = false;
    if addressing == imap::Addressing::Uid {
        append_item(
            &mut response,
            &mut first,
            "UID",
            message.uid.to_string().as_bytes(),
        );
        uid_written = true;
    }
    for attribute in attributes {
        if matches!(attribute, imap::FetchAttribute::Uid) && uid_written {
            continue;
        }
        if matches!(attribute, imap::FetchAttribute::Uid) {
            uid_written = true;
        }
        if let Err(reason) =
            append_attribute(&mut response, &mut first, session, message, attribute)
        {
            return Ok(Err(reason));
        }
        if response.len() > super::MAX_OUTPUT {
            return Ok(Err(crate::imap_fetch::FetchError::InvalidSection));
        }
    }
    response.extend_from_slice(b")\r\n");
    session.push(&response)?;
    Ok(Ok(()))
}

fn append_attribute(
    response: &mut Vec<u8>,
    first: &mut bool,
    session: &Session,
    message: &crate::store::Message,
    attribute: &imap::FetchAttribute,
) -> Result<(), crate::imap_fetch::FetchError> {
    match attribute {
        imap::FetchAttribute::Uid => {
            append_item(response, first, "UID", message.uid.to_string().as_bytes());
            Ok(())
        }
        imap::FetchAttribute::Rfc822 => {
            append_literal(response, first, "RFC822", &message.bytes);
            Ok(())
        }
        imap::FetchAttribute::Rfc822Header => append_legacy_section(
            response,
            first,
            message,
            "RFC822.HEADER",
            imap::BodySection::Headers { path: Vec::new() },
            true,
        ),
        imap::FetchAttribute::Rfc822Text => append_legacy_section(
            response,
            first,
            message,
            "RFC822.TEXT",
            imap::BodySection::Text { path: Vec::new() },
            false,
        ),
        imap::FetchAttribute::Rfc822Size => {
            append_item(
                response,
                first,
                "RFC822.SIZE",
                message.bytes.len().to_string().as_bytes(),
            );
            Ok(())
        }
        imap::FetchAttribute::BodyStructure => {
            let structure = crate::mime::tree::parse(&message.bytes)
                .and_then(|entity| crate::mime::bodystructure::encode(&entity, &message.bytes))
                .map_err(|_| crate::imap_fetch::FetchError::InvalidSection)?;
            append_item(response, first, "BODYSTRUCTURE", structure.as_bytes());
            Ok(())
        }
        imap::FetchAttribute::BodyStructureBasic => {
            let structure = crate::mime::tree::parse(&message.bytes)
                .and_then(|entity| crate::mime::bodystructure::encode(&entity, &message.bytes))
                .map_err(|_| crate::imap_fetch::FetchError::InvalidSection)?;
            append_item(response, first, "BODY", structure.as_bytes());
            Ok(())
        }
        imap::FetchAttribute::Body { .. } => {
            let data = crate::imap_fetch::select(&message.bytes, attribute)
                .map_err(|_| crate::imap_fetch::FetchError::InvalidSection)?;
            append_literal(response, first, &data.label, data.bytes.as_ref());
            Ok(())
        }
        imap::FetchAttribute::Binary { .. } => {
            let data = crate::imap_fetch::select_binary(&message.bytes, attribute)?;
            append_binary_literal(response, first, &data.label, data.bytes.as_ref());
            Ok(())
        }
        imap::FetchAttribute::BinarySize { section } => {
            let size = crate::imap_fetch::binary_size(&message.bytes, attribute)?;
            let label = format!(
                "BINARY.SIZE[{}]",
                section
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(".")
            );
            append_item(response, first, &label, size.to_string().as_bytes());
            Ok(())
        }
        imap::FetchAttribute::Envelope => {
            let envelope = crate::mime::envelope::encode(&message.bytes)
                .map_err(|_| crate::imap_fetch::FetchError::InvalidSection)?;
            append_item(response, first, "ENVELOPE", envelope.as_bytes());
            Ok(())
        }
        imap::FetchAttribute::Flags => {
            let flags = imap::flags::encode_stored(
                message.flags,
                &message.keywords,
                session.selected_recent.contains(&message.uid),
            );
            append_item(response, first, "FLAGS", format!("({flags})").as_bytes());
            Ok(())
        }
        imap::FetchAttribute::InternalDate => {
            let date = format!("\"{}\"", message.internal_date.encode());
            append_item(response, first, "INTERNALDATE", date.as_bytes());
            Ok(())
        }
    }
}

/// Сохраняет legacy wire label, переиспользуя собственный section selector.
fn append_legacy_section(
    response: &mut Vec<u8>,
    first: &mut bool,
    message: &crate::store::Message,
    label: &str,
    section: imap::BodySection,
    peek: bool,
) -> Result<(), crate::imap_fetch::FetchError> {
    let attribute = imap::FetchAttribute::Body {
        peek,
        section,
        partial: None,
    };
    let data = crate::imap_fetch::select(&message.bytes, &attribute)
        .map_err(|_| crate::imap_fetch::FetchError::InvalidSection)?;
    append_literal(response, first, label, data.bytes.as_ref());
    Ok(())
}

fn append_item(response: &mut Vec<u8>, first: &mut bool, label: &str, value: &[u8]) {
    append_separator(response, first);
    response.extend_from_slice(label.as_bytes());
    response.push(b' ');
    response.extend_from_slice(value);
}

fn append_literal(response: &mut Vec<u8>, first: &mut bool, label: &str, bytes: &[u8]) {
    append_separator(response, first);
    response.extend_from_slice(label.as_bytes());
    response.extend_from_slice(format!(" {{{}}}\r\n", bytes.len()).as_bytes());
    response.extend_from_slice(bytes);
}

/// BINARY response использует literal8, когда decoded данные содержат NUL.
fn append_binary_literal(response: &mut Vec<u8>, first: &mut bool, label: &str, bytes: &[u8]) {
    append_separator(response, first);
    response.extend_from_slice(label.as_bytes());
    let marker = match bytes.contains(&0) {
        true => format!(" ~{{{}}}\r\n", bytes.len()),
        false => format!(" {{{}}}\r\n", bytes.len()),
    };
    response.extend_from_slice(marker.as_bytes());
    response.extend_from_slice(bytes);
}

fn append_separator(response: &mut Vec<u8>, first: &mut bool) {
    if !*first {
        response.push(b' ');
    }
    *first = false;
}
