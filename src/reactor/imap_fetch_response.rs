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
) -> io::Result<Result<(), &'static str>> {
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
            return Ok(Err("FETCH response exceeds output limit"));
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
) -> Result<(), &'static str> {
    match attribute {
        imap::FetchAttribute::Uid => {
            append_item(response, first, "UID", message.uid.to_string().as_bytes());
            Ok(())
        }
        imap::FetchAttribute::Rfc822 => {
            append_literal(response, first, "RFC822", &message.bytes);
            Ok(())
        }
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
                .map_err(|_| "malformed MIME message")?;
            append_item(response, first, "BODYSTRUCTURE", structure.as_bytes());
            Ok(())
        }
        imap::FetchAttribute::Body { .. } => {
            let data = crate::imap_fetch::select(&message.bytes, attribute)
                .map_err(|_| "invalid BODY section")?;
            append_literal(response, first, &data.label, data.bytes);
            Ok(())
        }
        imap::FetchAttribute::Envelope => {
            let envelope = crate::mime::envelope::encode(&message.bytes)
                .map_err(|_| "malformed message headers")?;
            append_item(response, first, "ENVELOPE", envelope.as_bytes());
            Ok(())
        }
        imap::FetchAttribute::Flags => {
            let flags = imap::flags::encode_with_recent(
                message.flags,
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

fn append_separator(response: &mut Vec<u8>, first: &mut bool) {
    if !*first {
        response.push(b' ');
    }
    *first = false;
}
