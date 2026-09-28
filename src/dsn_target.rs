//! Цель committed DSN внутри Store и outbound queue.

/// Получатель отчёта после проверки локальной маршрутизации.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DsnTarget {
    /// Отправитель принадлежит существующему локальному account.
    LocalMailbox(String),
    /// Внешний reverse-path, который надо передать SMTP transport-у.
    OutboundAddress(String),
}
