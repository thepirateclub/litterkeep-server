//! Проверки RFC 6131 и локальных границ интервала vacation.

use super::{Action, VacationInterval, ast::Command, parse};

#[test]
fn vacation_seconds_capability_allows_seconds_without_base_capability() {
    let script = parse("require \"vacation-seconds\"; vacation :seconds 90 \"Away\";")
        .expect("RFC 6131 capability implies vacation");
    let Some(Action::Vacation(action)) = script.commands.first().and_then(action_of) else {
        panic!("typed vacation action expected");
    };
    assert_eq!(action.interval, Some(VacationInterval::Seconds(90)));
}

#[test]
fn vacation_seconds_requires_its_own_extension() {
    assert!(parse("require \"vacation\"; vacation :seconds 60 \"Away\";").is_err());
}

#[test]
fn vacation_seconds_rejects_values_outside_rfc_unsigned_range() {
    assert!(parse("require \"vacation-seconds\"; vacation :seconds 2147483647 \"Away\";").is_ok());
    assert!(parse("require \"vacation-seconds\"; vacation :seconds 2147483648 \"Away\";").is_err());
    assert!(parse("require \"vacation-seconds\"; vacation :seconds -1 \"Away\";").is_err());
    assert!(parse("require \"vacation-seconds\"; vacation :seconds +1 \"Away\";").is_err());
}

#[test]
fn vacation_days_and_seconds_are_mutually_exclusive() {
    assert!(parse("require \"vacation\"; vacation :days 1 :seconds 60 \"Away\";").is_err());
    assert!(parse("require \"vacation-seconds\"; vacation :seconds 60 :days 1 \"Away\";").is_err());
}

#[test]
fn effective_interval_applies_default_minimum_and_maximum_policy() {
    assert_eq!(super::vacation_policy::effective_seconds(None), 604_800);
    assert_eq!(
        super::vacation_policy::effective_seconds(Some(VacationInterval::Seconds(0))),
        60
    );
    assert_eq!(
        super::vacation_policy::effective_seconds(Some(VacationInterval::Seconds(90))),
        90
    );
    assert_eq!(
        super::vacation_policy::effective_seconds(Some(VacationInterval::Days(14))),
        1_209_600
    );
    assert_eq!(
        super::vacation_policy::effective_seconds(Some(VacationInterval::Days(u64::MAX))),
        365 * 86_400
    );
}

fn action_of(command: &Command) -> Option<&Action> {
    match command {
        Command::Action(action) => Some(action),
        Command::If { .. } => None,
    }
}
