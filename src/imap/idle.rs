//! Грамматика continuation активной IMAP IDLE команды.

/// Единственная допустимая continuation во время активного IDLE.
pub enum IdleContinuation {
    Done,
    Invalid,
}

/// Разбирает continuation отдельно от tagged command grammar.
pub fn parse_idle_continuation(line: &str) -> IdleContinuation {
    match line
        .trim_end_matches(['\r', '\n'])
        .to_ascii_uppercase()
        .as_str()
    {
        "DONE" => IdleContinuation::Done,
        _ => IdleContinuation::Invalid,
    }
}
