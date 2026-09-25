//! Локальные переходы закрытия и чтения для reactor session.
use super::Session;

/// Типизированная причина локального закрытия клиентского владельца.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionTermination {
    /// POP3 QUIT завершён; ответ и TLS close_notify должны уйти до cleanup.
    Pop3Quit,
    /// Отстающий IDLE-подписчик превысил жёсткую границу выходной очереди.
    NotificationOverflow,
    /// Некорректный UTF-8 SEARCH literal нарушил синтаксис текущей команды.
    InvalidSearchLiteral,
}

impl Session {
    /// Reactor читает только пока TLS или протоколу действительно нужен ввод.
    pub(in crate::reactor) fn needs_read(&self) -> bool {
        match (self.termination.is_some(), self.tls.as_ref()) {
            (true, _) => false,
            (false, Some(tls)) if tls.wants_read() => true,
            (false, Some(_) | None) => self.accepts_input(),
        }
    }

    /// Начинает alert только после отправки обычного protocol ответа.
    pub(in crate::reactor) fn begin_close_notify(&mut self) {
        match self.tls.as_mut() {
            Some(tls) if !self.close_notify_started => {
                tls.send_close_notify();
                self.close_notify_started = true;
            }
            Some(_) | None => {}
        }
    }
}
