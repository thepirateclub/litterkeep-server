//! Локальная политика SMTP envelope route без зависимости от reactor-а.
//!
//! Граница преобразует внешний адрес в локальный mailbox только для явно
//! разрешённых тестовых доменов. Дальше слои работают с именем account, а не
//! сравнивают domain strings повторно.

/// Преобразует список SMTP recipient paths в локальные mailbox names.
pub fn local_mailboxes(recipients: Vec<String>) -> Vec<String> {
    recipients
        .into_iter()
        .filter_map(|recipient| local_mailbox(&recipient))
        .collect()
}

/// Отделяет local-part от домена только для разрешённого локального маршрута.
pub fn local_mailbox(address: &str) -> Option<String> {
    let (mailbox, domain) = address.split_once('@')?;
    match LocalRoute::parse(domain) {
        LocalRoute::Accepted => Some(mailbox.to_owned()),
        LocalRoute::Rejected => None,
    }
}

/// Результат разбора domain на единственной string-to-enum границе.
enum LocalRoute {
    Accepted,
    Rejected,
}

impl LocalRoute {
    /// Локальные домены первого профиля без DNS/MX discovery.
    fn parse(domain: &str) -> Self {
        match domain {
            "test" | "litterkeep.test" | "peer.test" => Self::Accepted,
            _ => Self::Rejected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_domain_becomes_mailbox() {
        assert_eq!(local_mailbox("alice@litterkeep.test"), Some("alice".into()));
    }

    #[test]
    fn external_domain_is_not_a_local_mailbox() {
        assert_eq!(local_mailbox("alice@example.test"), None);
    }
}
