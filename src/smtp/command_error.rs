//! Таксономия отказов разбора SMTP-команд для выбора точного reply code.

/// Ошибки wire-грамматики SMTP сохраняются раздельно для ответа RFC 5321.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseError {
    /// Сервер не распознал имя SMTP-команды.
    UnrecognizedCommand,
    /// Сервер распознал команду, но её синтаксис или аргумент неверен.
    Syntax,
    /// Клиент запросил неизвестный параметр расширения.
    UnsupportedParameter,
    /// Клиент запросил неподдерживаемый SASL механизм.
    UnsupportedAuthenticationMechanism,
}

#[cfg(test)]
#[path = "command_error_tests.rs"]
mod tests;
