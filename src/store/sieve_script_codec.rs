//! Кодек долговечного snapshot Sieve catalog без файловых операций.

use super::{AccountScripts, ScriptRecord, invalid, script_name};
use crate::sieve;
use std::io;

pub(super) fn encoded(state: &AccountScripts) -> String {
    let mut output = format!(
        "next:{}\nactive:{}\n",
        state.next_version,
        state.active.as_deref().map(hex).unwrap_or_default()
    );
    for (name, record) in &state.scripts {
        output.push_str(&format!(
            "script:{}:{}:{}\n",
            hex(name),
            record.version,
            hex(&record.source)
        ));
    }
    output
}

pub(super) fn decode(source: &str) -> io::Result<AccountScripts> {
    let mut state = AccountScripts::default();
    for line in source.lines() {
        if let Some(value) = line.strip_prefix("next:") {
            state.next_version = value
                .parse()
                .map_err(|_| invalid("bad sieve next version"))?;
            continue;
        }
        if let Some(value) = line.strip_prefix("active:") {
            state.active = match value.is_empty() {
                true => None,
                false => Some(script_name::canonical(&unhex(value)?)?),
            };
            continue;
        }
        let Some(value) = line.strip_prefix("script:") else {
            return Err(invalid("bad sieve frame"));
        };
        let mut fields = value.split(':');
        let (Some(name), Some(version), Some(body), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(invalid("bad sieve script frame"));
        };
        let name = script_name::canonical(&unhex(name)?)?;
        let version = version.parse().map_err(|_| invalid("bad sieve version"))?;
        let body = unhex(body)?;
        sieve::parse(&body).map_err(invalid)?;
        if state.scripts.contains_key(&name) {
            return Err(invalid("canonical sieve script-name collision"));
        }
        state.scripts.insert(
            name,
            ScriptRecord {
                version,
                source: body,
            },
        );
    }
    Ok(state)
}

fn hex(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn unhex(value: &str) -> io::Result<String> {
    let mut output = Vec::new();
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    for pair in pairs {
        let text = std::str::from_utf8(pair).map_err(|_| invalid("non-utf8 sieve hex"))?;
        output.push(u8::from_str_radix(text, 16).map_err(|_| invalid("bad sieve hex"))?);
    }
    if !remainder.is_empty() {
        return Err(invalid("odd sieve hex"));
    }
    String::from_utf8(output).map_err(|_| invalid("non-utf8 sieve source"))
}
