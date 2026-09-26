//! Типизированная грамматика sync и non-sync literal-маркеров RFC 7888.

/// Форма передачи literal определяет наличие сетевой синхронизации.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    /// Клиент обязан дождаться серверного continuation перед octets.
    Synchronizing,
    /// Клиент передаёт octets сразу после CRLF и не ждёт continuation.
    NonSynchronizing,
}

/// Проверенный размер literal и его форма на wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Marker {
    pub size: usize,
    pub mode: Mode,
}

/// Разбирает `{octets}` и `{octets+}` без допуска знаков и пробелов.
pub fn parse(value: &str) -> Option<Marker> {
    let digits_and_mode = value.strip_prefix('{')?.strip_suffix('}')?;
    let (digits, mode) = match digits_and_mode.strip_suffix('+') {
        Some(digits) => (digits, Mode::NonSynchronizing),
        None => (digits_and_mode, Mode::Synchronizing),
    };
    match (
        digits.is_empty(),
        digits.bytes().all(|byte| byte.is_ascii_digit()),
    ) {
        (false, true) => Some(Marker {
            size: digits.parse().ok()?,
            mode,
        }),
        _ => None,
    }
}

#[cfg(test)]
#[path = "literal_tests.rs"]
mod tests;
