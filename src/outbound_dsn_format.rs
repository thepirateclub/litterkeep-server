//! Форматирование сохранённых SMTP DSN envelope для downstream peer.

use crate::smtp::{MailDsn, NotifyCondition, NotifyPolicy, OriginalAddressType, RecipientDsn};

/// MAIL параметры исходной transaction, которые разрешено ретранслировать.
pub(crate) fn mail_suffix(metadata: Option<&MailDsn>, dsn_supported: bool) -> String {
    match (metadata, dsn_supported) {
        (Some(metadata), true) => {
            let mut suffix = String::new();
            match metadata.return_content {
                Some(crate::smtp::ReturnContent::Full) => suffix.push_str(" RET=FULL"),
                Some(crate::smtp::ReturnContent::Headers) => suffix.push_str(" RET=HDRS"),
                None => {}
            }
            if let Some(envelope_id) = metadata.envelope_id.as_deref() {
                suffix.push_str(" ENVID=");
                suffix.push_str(&encode_xtext(envelope_id));
            }
            suffix
        }
        _ => String::new(),
    }
}

/// RCPT параметры одного исходного recipient без подмены его DSN policy.
pub(crate) fn recipient_suffix(
    metadata: Option<&RecipientDsn>,
    dsn_supported: bool,
) -> Option<String> {
    match (metadata, dsn_supported) {
        (Some(metadata), true) => {
            let mut suffix = String::new();
            if let Some(notify) = metadata.notify.as_ref() {
                suffix.push_str(" NOTIFY=");
                suffix.push_str(&notify_value(notify));
            }
            if let Some(original) = metadata.original_recipient.as_ref() {
                suffix.push_str(" ORCPT=");
                suffix.push_str(address_type(original.address_type));
                suffix.push(';');
                let address = match original.address_type {
                    OriginalAddressType::Rfc822 => encode_xtext(&original.address),
                    OriginalAddressType::Utf8 => encode_utf8_unitext(&original.address)?,
                };
                suffix.push_str(&address);
            }
            Some(suffix)
        }
        _ => Some(String::new()),
    }
}

/// Сериализует notify set в стабильном порядке, не зависящем от HashSet.
fn notify_value(policy: &NotifyPolicy) -> String {
    match policy {
        NotifyPolicy::Never => "NEVER".to_owned(),
        NotifyPolicy::Conditions(conditions) => [
            (NotifyCondition::Failure, "FAILURE"),
            (NotifyCondition::Success, "SUCCESS"),
            (NotifyCondition::Delay, "DELAY"),
        ]
        .into_iter()
        .filter_map(|(condition, value)| conditions.contains(&condition).then_some(value))
        .collect::<Vec<_>>()
        .join(","),
    }
}

/// Преобразует типизированный address-type в его зарегистрированное имя.
fn address_type(address_type: OriginalAddressType) -> &'static str {
    match address_type {
        OriginalAddressType::Rfc822 => "rfc822",
        OriginalAddressType::Utf8 => "utf-8",
    }
}

/// Экранирует octets по RFC 3461 xtext с заглавными hex цифрами.
fn encode_xtext(value: &[u8]) -> String {
    value.iter().fold(String::new(), |mut encoded, byte| {
        match byte {
            b'!'..=b'~' if *byte != b'+' && *byte != b'=' => {
                encoded.push(char::from(*byte));
            }
            _ => encoded.push_str(&format!("+{byte:02X}")),
        }
        encoded
    })
}

/// Кодирует UTF-8 ORCPT в 7-bit-safe RFC 6533 unitext.
fn encode_utf8_unitext(value: &[u8]) -> Option<String> {
    let value = std::str::from_utf8(value).ok()?;
    let mut encoded = String::new();
    for character in value.chars() {
        match character {
            '\0' | '\r' | '\n' => return None,
            '!'..='~' if !matches!(character, '\\' | '+' | '=') => encoded.push(character),
            _ => encoded.push_str(&format!("\\x{{{:X}}}", u32::from(character))),
        }
    }
    Some(encoded)
}

#[cfg(test)]
#[path = "outbound_dsn_format_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "outbound_dsn_wire_tests.rs"]
mod wire_tests;
