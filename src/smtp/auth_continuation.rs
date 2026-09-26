//! Классификация управляющих строк во время SMTP AUTH continuation.

/// Определяет RFC command interruption, не принимая Base64 за SMTP-команду.
pub fn interrupts_exchange(line: &[u8]) -> bool {
    let Some(value) = std::str::from_utf8(line)
        .ok()
        .and_then(|value| value.strip_suffix("\r\n"))
    else {
        return false;
    };
    value != "*" && (is_auth_command_line(value) || resembles_smtp_command(value))
}

/// Отделяет запрещённый вложенный AUTH от разрешённого прерывания другой командой.
pub fn is_nested_auth(line: &[u8]) -> bool {
    std::str::from_utf8(line)
        .ok()
        .and_then(|value| value.strip_suffix("\r\n"))
        .is_some_and(is_auth_command_line)
}

/// Повторный AUTH сравнивает только SMTP verb до первого разделителя.
fn is_auth_command_line(value: &str) -> bool {
    let verb = match value.split_once(' ') {
        Some((verb, _)) => verb,
        None => value,
    };
    verb.eq_ignore_ascii_case("AUTH")
}

/// Узнаёт команды текущего закрытого набора, чтобы не принять их за Base64.
fn resembles_smtp_command(value: &str) -> bool {
    let verb = match value.split_once(' ') {
        Some((verb, _)) => verb,
        None => value,
    };
    [
        "DATA", "EHLO", "EXPN", "HELO", "HELP", "MAIL", "NOOP", "QUIT", "RCPT", "RSET", "STARTTLS",
        "VRFY",
    ]
    .iter()
    .any(|candidate| verb.eq_ignore_ascii_case(candidate))
}

#[cfg(test)]
#[path = "auth_continuation_tests.rs"]
mod tests;
