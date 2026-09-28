//! Ограничение частоты повторных пакетных DSN sweep после ошибок Store.

use std::time::{Duration, Instant};

const INITIAL_DELAY: Duration = Duration::from_millis(100);
const MAXIMUM_DELAY: Duration = Duration::from_secs(30);

/// Экспоненциальная задержка действует на пакетный проход в пределах процесса.
pub(super) struct RetryDelay {
    next_attempt: Option<Instant>,
    current_delay: Duration,
}

impl RetryDelay {
    /// Создаёт состояние, готовое к первому проходу без ожидания.
    pub(super) fn new() -> Self {
        Self {
            next_attempt: None,
            current_delay: INITIAL_DELAY,
        }
    }

    /// Сообщает, наступил ли срок очередного пакетного прохода.
    pub(super) fn is_ready(&self, now: Instant) -> bool {
        match self.next_attempt {
            Some(deadline) => now >= deadline,
            None => true,
        }
    }

    /// Увеличивает задержку после сбоя, ограничивая её тридцатью секундами.
    pub(super) fn failed(&mut self, now: Instant) {
        self.next_attempt = Some(now + self.current_delay);
        self.current_delay = self.current_delay.saturating_mul(2).min(MAXIMUM_DELAY);
    }

    /// Сбрасывает задержку после успешного пакетного прохода.
    pub(super) fn succeeded(&mut self) {
        self.next_attempt = None;
        self.current_delay = INITIAL_DELAY;
    }
}

#[cfg(test)]
#[path = "retry_delay_tests.rs"]
mod tests;
