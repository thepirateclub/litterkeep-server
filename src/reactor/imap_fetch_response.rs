//! Кодирование одного untagged IMAP FETCH response из устойчивого snapshot.

use super::*;

pub(super) fn write(
    session: &mut Session,
    number: usize,
    message: &crate::store::Message,
    attribute: &imap::FetchAttribute,
    addressing: imap::Addressing,
) -> io::Result<Result<(), &'static str>> {
    let uid = match addressing {
        imap::Addressing::Sequence => String::new(),
        imap::Addressing::Uid => format!("UID {} ", message.uid),
    };
    match attribute {
        imap::FetchAttribute::Rfc822 => literal(session, number, &uid, "RFC822", &message.bytes),
        imap::FetchAttribute::Rfc822Size => {
            session.push(
                format!(
                    "* {number} FETCH ({uid}RFC822.SIZE {})\r\n",
                    message.bytes.len()
                )
                .as_bytes(),
            )?;
            Ok(Ok(()))
        }
        imap::FetchAttribute::BodyStructure => {
            let structure = crate::mime::tree::parse(&message.bytes)
                .and_then(|entity| crate::mime::bodystructure::encode(&entity, &message.bytes));
            match structure {
                Ok(structure) => {
                    session.push(
                        format!("* {number} FETCH ({uid}BODYSTRUCTURE {structure})\r\n").as_bytes(),
                    )?;
                    Ok(Ok(()))
                }
                Err(_) => Ok(Err("malformed MIME message")),
            }
        }
        imap::FetchAttribute::Body { .. } => {
            match crate::imap_fetch::select(&message.bytes, attribute) {
                Ok(data) => literal(session, number, &uid, &data.label, data.bytes),
                Err(_) => Ok(Err("invalid BODY section")),
            }
        }
        imap::FetchAttribute::Envelope => match crate::mime::envelope::encode(&message.bytes) {
            Ok(envelope) => {
                session
                    .push(format!("* {number} FETCH ({uid}ENVELOPE {envelope})\r\n").as_bytes())?;
                Ok(Ok(()))
            }
            Err(_) => Ok(Err("malformed message headers")),
        },
        imap::FetchAttribute::Flags => {
            let flags = imap::flags::encode_with_recent(
                message.flags,
                session.selected_recent.contains(&message.uid),
            );
            session.push(format!("* {number} FETCH ({uid}FLAGS ({flags}))\r\n").as_bytes())?;
            Ok(Ok(()))
        }
        imap::FetchAttribute::InternalDate => {
            let date = message.internal_date.encode();
            session
                .push(format!("* {number} FETCH ({uid}INTERNALDATE \"{date}\")\r\n").as_bytes())?;
            Ok(Ok(()))
        }
    }
}

fn literal(
    session: &mut Session,
    number: usize,
    uid: &str,
    label: &str,
    bytes: &[u8],
) -> io::Result<Result<(), &'static str>> {
    session.push(format!("* {number} FETCH ({uid}{label} {{{}}}\r\n", bytes.len()).as_bytes())?;
    session.push(bytes)?;
    session.push(b"\r\n)\r\n")?;
    Ok(Ok(()))
}
