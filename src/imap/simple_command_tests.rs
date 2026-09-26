//! Регрессионные проверки простых команд без аргументов.

use super::{Command, parse};

/// NOOP сохраняет типизированный вариант команды.
#[test]
fn noop_is_typed() {
    assert_eq!(parse("A8 NOOP\r\n").command, Command::Noop);
}
