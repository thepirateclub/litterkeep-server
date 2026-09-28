//! Локальная политика SMTP envelope route без зависимости от reactor-а.
//!
//! Граница преобразует внешний адрес в локальный mailbox только для явно
//! разрешённых тестовых доменов. Дальше слои работают с именем account, а не
//! сравнивают domain strings повторно.

/// Преобразует список SMTP recipient paths в локальные mailbox names.
pub fn local_mailboxes(recipients: Vec<String>) -> Vec<String> {
    recipients
        .into_iter()
        .filter_map(|recipient| local_recipient(&recipient))
        .collect()
}

/// Типизированная цель локальной SMTP-доставки.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LocalRecipient<'a> {
    Account(&'a str),
    Postmaster,
}

/// Проверяет mailbox адресата с учётом системной Postmaster доставки.
pub fn known_recipient(mailbox: &str, account_exists: impl FnOnce(&str) -> bool) -> bool {
    match classify_recipient_mailbox(mailbox) {
        LocalRecipient::Postmaster => true,
        LocalRecipient::Account(name) => account_exists(name),
    }
}

/// Разбирает Store mailbox к типизированной пользовательской или системной цели.
fn classify_recipient_mailbox(mailbox: &str) -> LocalRecipient<'_> {
    match mailbox.eq_ignore_ascii_case("postmaster") {
        true => LocalRecipient::Postmaster,
        false => LocalRecipient::Account(mailbox),
    }
}

/// Отделяет local-part от домена только для разрешённого локального маршрута.
pub fn local_mailbox(address: &str) -> Option<String> {
    let (mailbox, domain) = crate::smtp::canonical_parts(address)?;
    match LocalRoute::parse(&domain) {
        LocalRoute::Owned | LocalRoute::Peer => crate::accounts::canonical_query_name(&mailbox),
        LocalRoute::Rejected => None,
    }
}

/// Отличает собственные домены сервера от тестового peer route.
pub(crate) fn is_server_owned_domain(address: &str) -> bool {
    let Some((_, domain)) = crate::smtp::canonical_parts(address) else {
        return false;
    };
    matches!(LocalRoute::parse(&domain), LocalRoute::Owned)
}

/// Нормализует RCPT путь в локальный mailbox, включая bare Postmaster.
pub fn local_recipient(address: &str) -> Option<String> {
    match local_mailbox(address) {
        Some(mailbox) => match classify_recipient_mailbox(&mailbox) {
            LocalRecipient::Postmaster => Some("postmaster".to_owned()),
            LocalRecipient::Account(name) => Some(name.to_owned()),
        },
        None if !address.contains('@') => match crate::accounts::canonical_query_name(address) {
            Some(mailbox) => match classify_recipient_mailbox(&mailbox) {
                LocalRecipient::Postmaster => Some("postmaster".to_owned()),
                LocalRecipient::Account(_) => None,
            },
            None => None,
        },
        None => None,
    }
}

/// Нормализует протокольный логин в локальное имя владельца.
///
/// Короткое имя сохраняется для независимых клиентов и служебных fixtures.
/// Полный почтовый адрес принимается только для домена, разрешённого тем же
/// typed route-профилем, что и SMTP envelope. Поэтому внешний домен не может
/// превратиться в путь credential-файла или получить локальную сессию.
pub fn local_login(login: &str) -> Option<String> {
    match login.contains('@') {
        true => local_mailbox(login),
        false => crate::accounts::canonical_query_name(login),
    }
}

/// Результат разбора domain на единственной string-to-enum границе.
enum LocalRoute {
    Owned,
    Peer,
    Rejected,
}

impl LocalRoute {
    /// Локальные домены первого профиля без DNS/MX discovery.
    fn parse(domain: &str) -> Self {
        match domain.to_ascii_lowercase().as_str() {
            "test" | "litterkeep.test" | "xn--bcher-kva.test" => Self::Owned,
            "peer.test" => Self::Peer,
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
        assert_eq!(
            local_mailbox("\"alice\"@litterkeep.test"),
            Some("alice".into())
        );
        assert_eq!(local_mailbox("alice@bücher.test"), Some("alice".into()));
    }

    #[test]
    fn external_domain_is_not_a_local_mailbox() {
        assert_eq!(local_mailbox("alice@example.test"), None);
    }

    #[test]
    fn login_accepts_short_name_and_canonical_local_address() {
        assert_eq!(local_login("alice"), Some("alice".into()));
        assert_eq!(local_login("alice@litterkeep.test"), Some("alice".into()));
        assert_eq!(local_login("alice@example.test"), None);
    }

    /// SASLprep username и доставочный local-part разделяют account key.
    #[test]
    fn account_routes_apply_saslprep_before_storage_lookup() {
        assert_eq!(local_login("\u{00AA}"), Some("a".into()));
        assert_eq!(local_mailbox("\u{00AA}@litterkeep.test"), Some("a".into()));
        assert_eq!(local_mailbox("\u{0007}@litterkeep.test"), None);
        assert_eq!(local_login("\u{0627}1"), None);
    }

    /// Unicode-совместимая запись Postmaster не создаёт отдельный mailbox.
    #[test]
    fn normalized_postmaster_route_keeps_system_delivery() {
        assert_eq!(
            local_recipient("ＰＯＳＴＭＡＳＴＥＲ@test"),
            Some("postmaster".into())
        );
        assert_eq!(
            local_recipient("ＰＯＳＴＭＡＳＴＥＲ"),
            Some("postmaster".into())
        );
    }

    #[test]
    fn postmaster_routes_without_domain_and_case_insensitively() {
        for address in ["Postmaster", "POSTMASTER", "postmaster@test"] {
            assert_eq!(local_recipient(address), Some("postmaster".into()));
        }
        assert_eq!(local_recipient("postmaster@example.test"), None);
    }

    #[test]
    fn quoted_mailbox_is_canonicalized_before_local_account_lookup() {
        assert_eq!(local_recipient("\"bob\"@test"), Some("bob".into()));
        assert_eq!(local_recipient("\"b\\@b\"@test"), Some("b@b".into()));
        assert_eq!(local_recipient("\"bob\"@example.test"), None);
    }

    #[test]
    fn only_postmaster_bypasses_account_existence() {
        assert!(known_recipient("POSTMASTER", |_| false));
        assert!(!known_recipient("missing", |_| false));
        assert!(known_recipient("alice", |name| name == "alice"));
    }
}
