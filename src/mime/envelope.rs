//! Формирование IMAP ENVELOPE из структурированных RFC 5322 заголовков.

use super::encoded_word;
use super::envelope_address;
use super::header_block::{self, HeaderField, HeaderName};

/// Семантические поля ENVELOPE, используемые базовыми IMAP SEARCH keys.
#[derive(Debug, Default)]
pub(crate) struct SearchProjection {
    subject: Option<String>,
    from: Option<String>,
    to: Option<String>,
    cc: Option<String>,
    bcc: Option<String>,
}

impl SearchProjection {
    /// Возвращает текст одного поля без wire-разделителей RFC 5322 address-list.
    pub(crate) fn field(&self, name: HeaderName) -> Option<&str> {
        match name {
            HeaderName::Subject => self.subject.as_deref(),
            HeaderName::From => self.from.as_deref(),
            HeaderName::To => self.to.as_deref(),
            HeaderName::Cc => self.cc.as_deref(),
            HeaderName::Bcc => self.bcc.as_deref(),
            _ => None,
        }
    }
}

/// Строит ровно те декодированные поля, которые выдаются в IMAP ENVELOPE.
pub(crate) fn search_projection(fields: &[HeaderField]) -> SearchProjection {
    SearchProjection {
        subject: decoded_single(fields, HeaderName::Subject).ok().flatten(),
        from: address_search_text(fields, HeaderName::From),
        to: address_search_text(fields, HeaderName::To),
        cc: address_search_text(fields, HeaderName::Cc),
        bcc: address_search_text(fields, HeaderName::Bcc),
    }
}

fn address_search_text(fields: &[HeaderField], name: HeaderName) -> Option<String> {
    let parsed = envelope_address::parse(fields_for(fields, name)).ok()?;
    envelope_address::search_text(&parsed)
}

fn fields_for(fields: &[HeaderField], name: HeaderName) -> Vec<&HeaderField> {
    fields.iter().filter(|field| field.kind() == name).collect()
}

/// Кодирует десять обязательных полей IMAP ENVELOPE.
pub fn encode(message: &[u8]) -> Result<String, &'static str> {
    let headers = header_block::parse(message)?;
    let from = envelope_address::parse(fields(&headers.fields, HeaderName::From))?;
    let sender =
        envelope_address::with_default(fields(&headers.fields, HeaderName::Sender), &from)?;
    let reply_to =
        envelope_address::with_default(fields(&headers.fields, HeaderName::ReplyTo), &from)?;
    Ok(format!(
        "({} {} {} {} {} {} {} {} {} {})",
        nstring(single(&headers.fields, HeaderName::Date)?)?,
        nstring(decoded_single(&headers.fields, HeaderName::Subject)?)?,
        envelope_address::encode_list(&from)?,
        envelope_address::encode_list(&sender)?,
        envelope_address::encode_list(&reply_to)?,
        envelope_address::encode_list(&envelope_address::parse(fields(
            &headers.fields,
            HeaderName::To,
        ))?)?,
        envelope_address::encode_list(&envelope_address::parse(fields(
            &headers.fields,
            HeaderName::Cc,
        ))?)?,
        envelope_address::encode_list(&envelope_address::parse(fields(
            &headers.fields,
            HeaderName::Bcc,
        ))?)?,
        nstring(single(&headers.fields, HeaderName::InReplyTo)?)?,
        nstring(single(&headers.fields, HeaderName::MessageId)?)?,
    ))
}

fn fields(fields: &[HeaderField], kind: HeaderName) -> Vec<&HeaderField> {
    fields.iter().filter(|field| field.kind() == kind).collect()
}

fn single(fields: &[HeaderField], kind: HeaderName) -> Result<Option<String>, &'static str> {
    let Some(field) = fields.iter().find(|field| field.kind() == kind) else {
        return Ok(None);
    };
    let value = std::str::from_utf8(&field.value).map_err(|_| "ENVELOPE header is not UTF-8")?;
    Ok(Some(value.to_owned()))
}

fn decoded_single(
    fields: &[HeaderField],
    kind: HeaderName,
) -> Result<Option<String>, &'static str> {
    let Some(value) = single(fields, kind)? else {
        return Ok(None);
    };
    Ok(Some(encoded_word::decode_header(&value)?))
}

fn nstring(value: Option<String>) -> Result<String, &'static str> {
    let Some(value) = value else {
        return Ok("NIL".to_owned());
    };
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '\r' | '\n' => return Err("newline in ENVELOPE value"),
            '\\' | '"' => {
                output.push('\\');
                output.push(character);
            }
            _ => output.push(character),
        }
    }
    output.push('"');
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_addresses_and_decoded_subject() {
        let raw = b"Date: Fri, 19 Sep 2026 20:00:00 +0300\r\nSubject: =?UTF-8?B?0J/RgNC40LLQtdGC?=\r\nFrom: Alice <alice@test>\r\nTo: Bob <bob@test>\r\nMessage-ID: <one@test>\r\n\r\nbody";
        let envelope = encode(raw).expect("envelope");
        assert!(envelope.contains("\"Привет\""));
        assert!(envelope.contains("(\"Alice\" NIL \"alice\" \"test\")"));
        assert!(envelope.contains("(\"Bob\" NIL \"bob\" \"test\")"));
    }
}
