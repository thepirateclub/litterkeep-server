//! Извлечение ограниченных address-проекций для Sieve `address` test.
//!
//! Этот слой не решает SMTP delivery и не выдаёт filesystem paths. Он лишь
//! отделяет список адресов RFC 5322 от typed matcher-а, сохраняя bounded число
//! кандидатов даже для намеренно большого заголовка.

use super::ast::AddressPart;

const MAX_ADDRESSES: usize = 64;

/// Возвращает допустимые части mailbox из одного заголовочного значения.
pub(super) fn values(header: &str, part: AddressPart) -> Vec<String> {
    let mut result = Vec::new();
    for item in split(header).into_iter().take(MAX_ADDRESSES) {
        let Some(address) = address(item) else {
            continue;
        };
        if let Some(value) = project(address, part) {
            result.push(value);
        }
    }
    result
}

fn split(value: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0_usize;
    let mut quoted = false;
    let mut escaped = false;
    let mut angle = false;
    for (position, character) in value.char_indices() {
        match (escaped, character) {
            (true, _) => escaped = false,
            (false, '\\') if quoted => escaped = true,
            (false, '"') => quoted = !quoted,
            (false, '<') if !quoted => angle = true,
            (false, '>') if !quoted => angle = false,
            (false, ',') if matches!((quoted, angle), (false, false)) => {
                if let Some(item) = value.get(start..position) {
                    result.push(item);
                }
                start = position.saturating_add(1);
            }
            _ => {}
        }
    }
    if let Some(item) = value.get(start..) {
        result.push(item);
    }
    result
}

struct Mailbox<'a> {
    local: &'a str,
    domain: &'a str,
}

fn address(value: &str) -> Option<Mailbox<'_>> {
    let trimmed = value.trim();
    let source = match (trimmed.find('<'), trimmed.rfind('>')) {
        (Some(open), Some(close)) => trimmed.get(open.saturating_add(1)..close)?,
        (None, None) => trimmed,
        _ => return None,
    };
    let (local, domain) = source.trim().split_once('@')?;
    match (
        local.is_empty(),
        domain.is_empty(),
        source.contains(char::is_whitespace),
        local.contains('@'),
        domain.contains('@'),
    ) {
        (false, false, false, false, false) => Some(Mailbox { local, domain }),
        _ => None,
    }
}

fn project(mailbox: Mailbox<'_>, part: AddressPart) -> Option<String> {
    // Политика сервера использует первый `+` как разделитель RFC 5233.
    let (user, detail) = match mailbox.local.split_once('+') {
        Some((user, detail)) => (user, Some(detail)),
        None => (mailbox.local, None),
    };
    Some(match part {
        AddressPart::All => format!("{}@{}", mailbox.local, mailbox.domain),
        AddressPart::LocalPart => mailbox.local.to_owned(),
        AddressPart::Domain => mailbox.domain.to_owned(),
        AddressPart::User => user.to_owned(),
        AddressPart::Detail => return detail.map(str::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_display_name_does_not_split_the_address_list() {
        let values = values(
            "\"Alice, Example\" <alice+sales@example.test>, bob@example.test",
            AddressPart::Detail,
        );
        assert_eq!(values, vec!["sales"]);
    }

    #[test]
    fn explicit_empty_detail_matches_but_missing_detail_does_not() {
        assert_eq!(values("alice+@example.test", AddressPart::Detail), vec![""]);
        assert!(values("alice@example.test", AddressPart::Detail).is_empty());
    }
}
