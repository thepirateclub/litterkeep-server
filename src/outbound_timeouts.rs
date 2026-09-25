//! Нормативные per-command таймауты исходящего SMTP-клиента.

use super::ReplyExpectation;
use std::time::Duration;

/// Таймауты отдельной SMTP-фазы и передачи DATA.
#[derive(Clone, Copy)]
pub(crate) struct SmtpTimeouts {
    greeting: Duration,
    command: Duration,
    data_initiation: Duration,
    data_block: Duration,
    data_termination: Duration,
}

impl SmtpTimeouts {
    /// Создаёт одинаковые интервалы только для ограниченных тестовых peer-ов.
    #[cfg(test)]
    pub(crate) fn uniform(interval: Duration) -> Self {
        Self {
            greeting: interval,
            command: interval,
            data_initiation: interval,
            data_block: interval,
            data_termination: interval,
        }
    }

    /// Выбирает ожидание ответа по текущей SMTP-команде.
    pub(crate) fn response(self, expectation: ReplyExpectation) -> Duration {
        match expectation {
            ReplyExpectation::Greeting => self.greeting,
            ReplyExpectation::Data => self.data_initiation,
            ReplyExpectation::Message => self.data_termination,
            _ => self.command,
        }
    }

    /// Возвращает предел ожидания завершения каждой системной отправки DATA.
    pub(crate) fn data_block(self) -> Duration {
        self.data_block
    }

    /// Возвращает предел ожидания записи отдельной SMTP-команды.
    pub(crate) fn command(self) -> Duration {
        self.command
    }

    /// Возвращает предел ожидания initial greeting до обработки EHLO.
    pub(crate) fn greeting(self) -> Duration {
        self.greeting
    }
}

impl Default for SmtpTimeouts {
    /// Значения соответствуют рекомендуемым минимумам RFC 5321 §4.5.3.2.
    fn default() -> Self {
        Self {
            greeting: Duration::from_secs(5 * 60),
            command: Duration::from_secs(5 * 60),
            data_initiation: Duration::from_secs(2 * 60),
            data_block: Duration::from_secs(3 * 60),
            data_termination: Duration::from_secs(10 * 60),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SmtpTimeouts;
    use crate::outbound::ReplyExpectation;
    use std::time::Duration;

    #[test]
    fn reply_timeout_is_selected_by_smtp_phase() {
        let timeouts = SmtpTimeouts::default();
        assert_eq!(
            timeouts.response(ReplyExpectation::Greeting),
            Duration::from_secs(300)
        );
        assert_eq!(
            timeouts.response(ReplyExpectation::Mail),
            Duration::from_secs(300)
        );
        assert_eq!(
            timeouts.response(ReplyExpectation::Recipient),
            Duration::from_secs(300)
        );
        assert_eq!(
            timeouts.response(ReplyExpectation::Data),
            Duration::from_secs(120)
        );
        assert_eq!(timeouts.data_block(), Duration::from_secs(180));
        assert_eq!(
            timeouts.response(ReplyExpectation::Message),
            Duration::from_secs(600)
        );
    }
}
