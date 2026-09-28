//! Цель committed positive DSN внутри Store и outbound queue.

/// Получатель positive report после проверки локальной маршрутизации.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DsnSuccessTarget {
    /// Отправитель принадлежит существующему локальному account.
    LocalMailbox(String),
    /// Внешний reverse-path, который надо передать SMTP transport-у.
    OutboundAddress(String),
}
