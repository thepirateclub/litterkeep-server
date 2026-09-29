//! Извлечение ограниченных address-проекций для Sieve `address` test.
//!
//! Этот слой использует общий RFC 5322 parser и отделяет mailbox-проекции от
//! typed matcher-а, сохраняя bounded число кандидатов из большого заголовка.

use super::ast::AddressPart;

/// Возвращает допустимые части mailbox из одного заголовочного значения.
pub(super) fn values(header: &str, part: AddressPart) -> Vec<String> {
    match crate::mime::envelope_address::parse_value(header) {
        Ok(mailboxes) => mailboxes
            .into_iter()
            .filter_map(|mailbox| project(mailbox.local, mailbox.domain, part))
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Проецирует SMTP envelope address; null reverse-path всегда представлен пустой строкой.
pub(super) fn envelope_value(address: &str, part: AddressPart) -> Vec<String> {
    match address.is_empty() {
        true => vec![String::new()],
        false => values(address, part),
    }
}

fn project(local: String, domain: String, part: AddressPart) -> Option<String> {
    // Политика сервера использует первый `+` как разделитель RFC 5233.
    let (user, detail) = match local.split_once('+') {
        Some((user, detail)) => (user.to_owned(), Some(detail.to_owned())),
        None => (local.clone(), None),
    };
    Some(match part {
        AddressPart::All => format!("{local}@{domain}"),
        AddressPart::LocalPart => local,
        AddressPart::Domain => domain,
        AddressPart::User => user,
        AddressPart::Detail => return detail,
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

    #[test]
    fn structured_group_returns_members_without_group_name() {
        assert_eq!(
            values(
                "Billing: alice@example.test, bob@example.test;",
                AddressPart::All
            ),
            vec!["alice@example.test", "bob@example.test"]
        );
    }

    /// RFC 5228 требует проверять каждый mailbox без скрытого предела в 64 записи.
    #[test]
    fn address_projection_includes_mailboxes_after_the_old_limit() {
        let header = (0..65)
            .map(|number| format!("user{number}@example.test"))
            .collect::<Vec<_>>()
            .join(", ");

        let parsed = values(&header, AddressPart::All);

        assert_eq!(parsed.len(), 65);
        assert_eq!(
            parsed.last().map(String::as_str),
            Some("user64@example.test")
        );
    }
}
