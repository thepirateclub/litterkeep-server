//! Локальная политика назначения SMTP envelope в существующие ящики.
//!
//! Этот модуль намеренно не знает о сокете, транзакции SMTP и файловом
//! хранилище. Он переводит внешний текстовый адрес в типизированное решение,
//! после чего reactor может передать только имя mailbox владельцу хранилища.

/// Преобразует принятые SMTP path в имена локальных ящиков текущего узла.
///
/// Домен остаётся входом для политики маршрутизации. Здесь он не отбрасывается
/// молча: адрес с внешним либо некорректным доменом не попадает в delivery.
pub(super) fn local_mailboxes(recipients: Vec<String>) -> Vec<String> {
    recipients
        .into_iter()
        .filter_map(|recipient| local_mailbox(&recipient))
        .collect()
}

/// Отделяет local-part от домена и возвращает имя ящика только для local route.
pub(super) fn local_mailbox(address: &str) -> Option<String> {
    let (mailbox, domain) = address.split_once('@')?;
    match LocalRoute::parse(domain) {
        LocalRoute::Accepted => Some(mailbox.to_owned()),
        LocalRoute::Rejected => None,
    }
}

/// Результат разбора домена на границе SMTP envelope.
///
/// Строковое сравнение изолировано в parser boundary. За этой границей код
/// работает с enum, поэтому внешний адрес нельзя ошибочно считать mailbox.
enum LocalRoute {
    Accepted,
    Rejected,
}

impl LocalRoute {
    /// Локальные тестовые домены первого профиля без DNS и MX discovery.
    fn parse(domain: &str) -> Self {
        match domain {
            "test" | "litterkeep.test" => Self::Accepted,
            _ => Self::Rejected,
        }
    }
}
