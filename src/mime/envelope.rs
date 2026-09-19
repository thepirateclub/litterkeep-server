//! Формирование IMAP ENVELOPE из структурированных RFC 5322 заголовков.

use super::encoded_word;
use super::header_block::{self, HeaderField, HeaderName};

#[derive(Debug, Eq, PartialEq)]
struct Address {
    name: Option<String>,
    mailbox: String,
    host: String,
}

/// Кодирует десять обязательных полей IMAP ENVELOPE.
pub fn encode(message: &[u8]) -> Result<String, &'static str> {
    let headers = header_block::parse(message)?;
    let from = addresses(fields(&headers.fields, HeaderName::From))?;
    let sender = addresses_or_default(fields(&headers.fields, HeaderName::Sender), &from)?;
    let reply_to = addresses_or_default(fields(&headers.fields, HeaderName::ReplyTo), &from)?;
    Ok(format!(
        "({} {} {} {} {} {} {} {} {} {})",
        nstring(single(&headers.fields, HeaderName::Date)?)?,
        nstring(decoded_single(&headers.fields, HeaderName::Subject)?)?,
        address_list(&from)?,
        address_list(&sender)?,
        address_list(&reply_to)?,
        address_list(&addresses(fields(&headers.fields, HeaderName::To))?)?,
        address_list(&addresses(fields(&headers.fields, HeaderName::Cc))?)?,
        address_list(&addresses(fields(&headers.fields, HeaderName::Bcc))?)?,
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
    match encoded_word::decode(&value) {
        Ok(decoded) => Ok(Some(decoded)),
        Err(_) => Ok(Some(value)),
    }
}

fn addresses(fields: Vec<&HeaderField>) -> Result<Vec<Address>, &'static str> {
    let mut result = Vec::new();
    for field in fields {
        let value = std::str::from_utf8(&field.value).map_err(|_| "address header is not UTF-8")?;
        for item in split_addresses(value)? {
            result.push(parse_address(item.trim())?);
        }
    }
    Ok(result)
}

fn addresses_or_default(
    fields: Vec<&HeaderField>,
    fallback: &[Address],
) -> Result<Vec<Address>, &'static str> {
    let parsed = addresses(fields)?;
    match parsed.is_empty() {
        true => Ok(fallback.to_vec()),
        false => Ok(parsed),
    }
}

fn split_addresses(value: &str) -> Result<Vec<&str>, &'static str> {
    let mut result = Vec::new();
    let mut quoted = false;
    let mut angle = false;
    let mut start = 0_usize;
    for (position, character) in value.char_indices() {
        match character {
            '"' => quoted = !quoted,
            '<' if !quoted => angle = true,
            '>' if !quoted => angle = false,
            ',' if matches!((quoted, angle), (false, false)) => {
                result.push(
                    value
                        .get(start..position)
                        .ok_or("invalid address boundary")?,
                );
                start = position.saturating_add(1);
            }
            _ => {}
        }
    }
    match (quoted, angle) {
        (true, _) | (false, true) => return Err("unterminated address syntax"),
        (false, false) => {}
    }
    result.push(value.get(start..).ok_or("invalid address tail")?);
    Ok(result)
}

fn parse_address(value: &str) -> Result<Address, &'static str> {
    let (name, addr_spec) = match (value.find('<'), value.rfind('>')) {
        (Some(open), Some(close)) => {
            let name = value.get(..open).ok_or("invalid display name")?.trim();
            let address = value
                .get(open.saturating_add(1)..close)
                .ok_or("invalid angle address")?;
            (decode_name(name)?, address)
        }
        (None, None) => (None, value.trim()),
        _ => return Err("unbalanced angle address"),
    };
    let (mailbox, host) = addr_spec.split_once('@').ok_or("address has no domain")?;
    if mailbox.is_empty() {
        return Err("address mailbox is empty");
    }
    if host.is_empty() {
        return Err("address host is empty");
    }
    Ok(Address {
        name,
        mailbox: mailbox.to_owned(),
        host: host.to_owned(),
    })
}

fn decode_name(value: &str) -> Result<Option<String>, &'static str> {
    let unquoted = value.trim_matches('"').trim();
    if unquoted.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        encoded_word::decode(unquoted).unwrap_or_else(|_| unquoted.to_owned()),
    ))
}

fn address_list(addresses: &[Address]) -> Result<String, &'static str> {
    if addresses.is_empty() {
        return Ok("NIL".to_owned());
    }
    let mut output = String::from("(");
    for address in addresses {
        output.push_str(&format!(
            "({} NIL {} {})",
            nstring(address.name.clone())?,
            nstring(Some(address.mailbox.clone()))?,
            nstring(Some(address.host.clone()))?
        ));
    }
    output.push(')');
    Ok(output)
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

impl Clone for Address {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            mailbox: self.mailbox.clone(),
            host: self.host.clone(),
        }
    }
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
