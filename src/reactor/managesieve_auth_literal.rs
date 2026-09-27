//! Распознавание SASL initial-response literal в ManageSieve AUTHENTICATE.

use super::{Operation, Start, literal, quoted_then_tail};

/// Поддерживаемые здесь SASL-механизмы после разбора протокольной строки.
enum Mechanism {
    Plain,
    Unsupported,
}

impl Mechanism {
    /// Преобразует внешнее имя механизма в типизированный вариант.
    fn parse(value: &str) -> Self {
        match value.eq_ignore_ascii_case("PLAIN") {
            true => Self::Plain,
            false => Self::Unsupported,
        }
    }
}

/// Возвращает receiver только для quoted PLAIN mechanism и literal response.
pub(super) fn start(tail: &str) -> Start {
    let Some((mechanism, marker)) = quoted_then_tail(tail) else {
        return Start::NotLiteral;
    };
    match Mechanism::parse(&mechanism) {
        Mechanism::Plain => literal(Operation::AuthenticatePlain, marker),
        Mechanism::Unsupported => Start::NotLiteral,
    }
}
