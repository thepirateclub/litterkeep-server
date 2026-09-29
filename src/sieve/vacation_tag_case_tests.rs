//! Регрессии ASCII-регистра tagged arguments расширения vacation.

use crate::sieve::ast::{Action, Command};
use crate::sieve::parse;

/// RFC 5228 keywords и tagged arguments не зависят от ASCII-регистра.
#[test]
fn vacation_parameters_are_ascii_case_insensitive() {
    let script = parse(
        "ReQuIrE \"vacation\"; VaCaTiOn :DAYS 3 :SuBjEcT \"Away\" :HaNdLe \"trip\" \"Gone\";",
    )
    .expect("mixed-case vacation syntax");
    let Some(Command::Action(Action::Vacation(action))) = script.commands.first() else {
        panic!("typed vacation action expected");
    };

    assert_eq!(
        action.interval,
        Some(crate::sieve::ast::VacationInterval::Days(3))
    );
    assert_eq!(action.subject.as_deref(), Some("Away"));
    assert_eq!(action.handle.as_deref(), Some("trip"));
    assert_eq!(action.reason, "Gone");
}

/// Capability identifiers в отличие от остальных токенов чувствительны к регистру.
#[test]
fn vacation_capability_identifier_remains_case_sensitive() {
    assert!(crate::sieve::parse("require \"VACATION\"; vacation \"Away\";").is_err());
}
