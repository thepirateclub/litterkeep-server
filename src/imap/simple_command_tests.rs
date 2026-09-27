//! Регрессионные проверки простых команд без аргументов.

use super::{Command, parse};

/// NOOP сохраняет типизированный вариант команды.
#[test]
fn noop_is_typed() {
    assert_eq!(parse("A8 NOOP\r\n").command, Command::Noop);
}

/// ID передаётся как типизированная команда только после проверки параметров.
#[test]
fn id_is_typed_and_invalid_arguments_are_rejected() {
    assert_eq!(parse("A9 ID NIL\r\n").command, Command::Id);
    assert_eq!(parse("B9 ID ()\r\n").command, Command::Id);
    assert_eq!(
        parse("C9 ID (\"name\" \"client\")\r\n").command,
        Command::Id
    );
    assert_eq!(parse("D9 ID (\"name\")\r\n").command, Command::Unsupported);
}
