//! Runtime-регрессии RFC 5229 `set` с execution-local состоянием.

use super::{ExecutionError, execute_state};
use crate::sieve::parse;

#[test]
fn set_expands_current_values_applies_modifiers_and_preserves_implicit_keep() {
    let script = parse(
        "require \"variables\"; set :upperfirst :lower \"Greeting\" \"hELLO\"; set \"copy\" \"${greeting}!\";",
    )
    .expect("valid Variables script");
    let (plan, variables) = execute_state(&script, &[], 0, "", "").expect("execute set");

    assert!(plan.keep);
    assert_eq!(variables.get("greeting"), Some("Hello"));
    assert_eq!(variables.get("COPY"), Some("Hello!"));
}

#[test]
fn set_expands_match_capture_at_the_action_statement() {
    let script = parse(
        "require \"variables\"; if header :matches \"Subject\" \"[*]\" { set \"label\" \"${1}\"; }",
    )
    .expect("valid capture script");
    let headers = vec![("Subject".to_owned(), "[Invoices]".to_owned())];
    let (_, variables) = execute_state(&script, &headers, 0, "", "").expect("capture is UTF-8");

    assert_eq!(variables.get("label"), Some("Invoices"));
}

#[test]
fn unused_non_utf8_octet_capture_does_not_fail_constant_set() {
    let script = parse(
        "require \"variables\"; if header :matches \"Subject\" \"??\" { set \"safe\" \"constant\"; }",
    )
    .expect("valid octet capture script");
    let headers = vec![("Subject".to_owned(), "é".to_owned())];
    let (_, variables) = execute_state(&script, &headers, 0, "", "").expect("capture is unused");

    assert_eq!(variables.get("safe"), Some("constant"));
}

#[test]
fn referenced_non_utf8_octet_capture_fails_without_lossy_conversion() {
    let script = parse(
        "require \"variables\"; if header :matches \"Subject\" \"??\" { set \"unsafe\" \"${1}\"; }",
    )
    .expect("valid octet capture script");
    let headers = vec![("Subject".to_owned(), "é".to_owned())];

    assert_eq!(
        execute_state(&script, &headers, 0, "", "").map(|_| ()),
        Err(ExecutionError::VariableRuntime)
    );
}
