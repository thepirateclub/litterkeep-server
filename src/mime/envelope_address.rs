//! Разбор RFC 5322 address-list и проекция адресных групп в IMAP ENVELOPE.

use super::encoded_word;
use super::envelope_address_scan::{
    normalize_dot_cfws, split_addr_spec, split_commas, split_optional, strip_comments,
    strip_source_route,
};
use super::header_block::HeaderField;

/// Обычный mailbox после отделения его имени и домена.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Address {
    pub(super) name: Option<String>,
    pub(super) mailbox: String,
    pub(super) host: String,
}

/// Типизированный элемент address-list, включая RFC 5322 group boundaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum AddressEntry {
    Mailbox(Address),
    GroupStart(String),
    GroupEnd,
}

/// Разбирает повторяющиеся поля в исходном порядке, не теряя пустые группы.
pub(super) fn parse(fields: Vec<&HeaderField>) -> Result<Vec<AddressEntry>, &'static str> {
    let mut entries = Vec::new();
    for field in fields {
        let value = std::str::from_utf8(&field.value).map_err(|_| "address header is not UTF-8")?;
        parse_list(value, &mut entries)?;
    }
    Ok(entries)
}

/// При пустых Sender/Reply-To использует уже разобранный From согласно IMAP.
pub(super) fn with_default(
    fields: Vec<&HeaderField>,
    fallback: &[AddressEntry],
) -> Result<Vec<AddressEntry>, &'static str> {
    let parsed = parse(fields)?;
    match parsed.is_empty() {
        true => Ok(fallback.to_vec()),
        false => Ok(parsed),
    }
}

/// Кодирует mailbox и обе границы группы в последовательность IMAP address.
pub(super) fn encode_list(entries: &[AddressEntry]) -> Result<String, &'static str> {
    if entries.is_empty() {
        return Ok("NIL".to_owned());
    }
    let mut output = String::from("(");
    for entry in entries {
        let encoded = match entry {
            AddressEntry::Mailbox(address) => format!(
                "({} NIL {} {})",
                nstring(address.name.clone())?,
                nstring(Some(address.mailbox.clone()))?,
                nstring(Some(address.host.clone()))?
            ),
            AddressEntry::GroupStart(name) => {
                format!("(NIL NIL {} NIL)", nstring(Some(name.clone()))?)
            }
            AddressEntry::GroupEnd => "(NIL NIL NIL NIL)".to_owned(),
        };
        output.push_str(&encoded);
    }
    output.push(')');
    Ok(output)
}

/// Проецирует только ENVELOPE components для FROM/TO/CC/BCC поиска.
pub(super) fn search_text(entries: &[AddressEntry]) -> Option<String> {
    let mut values = Vec::new();
    for entry in entries {
        match entry {
            AddressEntry::Mailbox(address) => {
                values.extend(address_terms(address));
            }
            AddressEntry::GroupStart(name) => values.push(name.clone()),
            AddressEntry::GroupEnd => {}
        }
    }
    match values.is_empty() {
        true => None,
        false => Some(values.join(" ")),
    }
}

fn parse_list(value: &str, entries: &mut Vec<AddressEntry>) -> Result<(), &'static str> {
    let mut in_group = false;
    for item in split_commas(value)? {
        match in_group {
            true => {
                let (member, closing) = split_optional(item, ';')?;
                append_mailbox(member, entries)?;
                if closing.is_some() {
                    entries.push(AddressEntry::GroupEnd);
                    in_group = false;
                }
            }
            false => match (split_optional(item, ':')?, split_optional(item, ';')?) {
                ((group_name, Some(members)), _) => {
                    let group_name =
                        decode_name(group_name.trim())?.ok_or("empty address group name")?;
                    entries.push(AddressEntry::GroupStart(group_name));
                    in_group = true;
                    let (member, closing) = split_optional(members, ';')?;
                    append_mailbox(member, entries)?;
                    if closing.is_some() {
                        entries.push(AddressEntry::GroupEnd);
                        in_group = false;
                    }
                }
                ((_mailbox, None), (_, Some(_))) => {
                    return Err("group terminator without group");
                }
                ((mailbox, None), (_, None)) => append_mailbox(mailbox, entries)?,
            },
        }
    }
    match in_group {
        true => Err("unterminated address group"),
        false => Ok(()),
    }
}

fn append_mailbox(value: &str, entries: &mut Vec<AddressEntry>) -> Result<(), &'static str> {
    let value = value.trim();
    match value.is_empty() {
        true => Ok(()),
        false => {
            entries.push(AddressEntry::Mailbox(parse_mailbox(value)?));
            Ok(())
        }
    }
}

fn parse_mailbox(value: &str) -> Result<Address, &'static str> {
    let value = strip_comments(value)?;
    let (name, addr_spec) = match (value.find('<'), value.rfind('>')) {
        (Some(open), Some(close)) => {
            let name = value.get(..open).ok_or("invalid display name")?.trim();
            let address = value
                .get(open.saturating_add(1)..close)
                .ok_or("invalid angle address")?;
            (decode_name(name)?, strip_source_route(address)?)
        }
        (None, None) => (None, value.trim()),
        _ => return Err("unbalanced angle address"),
    };
    let addr_spec = strip_comments(addr_spec)?;
    let (mailbox, host) = split_addr_spec(&addr_spec)?;
    let mailbox = normalize_dot_cfws(mailbox.trim());
    let host = normalize_dot_cfws(host.trim());
    if mailbox.is_empty() {
        return Err("address mailbox is empty");
    }
    if host.is_empty() {
        return Err("address host is empty");
    }
    Ok(Address {
        name,
        mailbox,
        host,
    })
}

fn address_terms(address: &Address) -> Vec<String> {
    let addr_spec = format!("{}@{}", address.mailbox, address.host);
    [
        address.name.clone(),
        Some(address.mailbox.clone()),
        Some(address.host.clone()),
        Some(addr_spec),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn decode_name(value: &str) -> Result<Option<String>, &'static str> {
    let value = strip_comments(value)?;
    let unquoted = value.trim();
    if unquoted.is_empty() {
        return Ok(None);
    }
    let decoded = match unquoted
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
    {
        Some(inner) => decode_quoted_phrase(inner)?,
        None => encoded_word::decode_phrase(&normalize_phrase(unquoted)),
    };
    Ok(Some(decoded))
}

/// Удаляет quoting backslash из RFC 5322 quoted-pair внутри phrase.
fn decode_quoted_phrase(value: &str) -> Result<String, &'static str> {
    let mut characters = value.chars();
    let mut decoded = String::with_capacity(value.len());
    while let Some(character) = characters.next() {
        match character {
            '\\' => match characters.next() {
                Some(quoted) => decoded.push(quoted),
                None => return Err("dangling quoted-pair in display name"),
            },
            _ => decoded.push(character),
        }
    }
    Ok(decoded)
}

/// Сворачивает CFWS между словами personal/group phrase в один пробел.
fn normalize_phrase(value: &str) -> String {
    let mut phrase = String::with_capacity(value.len());
    let mut pending_space = false;
    for character in value.chars() {
        match character.is_whitespace() {
            true if !phrase.is_empty() => pending_space = true,
            true => {}
            false => {
                if pending_space {
                    phrase.push(' ');
                }
                phrase.push(character);
                pending_space = false;
            }
        }
    }
    phrase
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
#[path = "envelope_address_tests.rs"]
mod tests;
