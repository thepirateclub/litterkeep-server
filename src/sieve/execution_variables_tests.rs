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

#[test]
fn string_test_expands_variables_and_preserves_script_whitespace() {
    let source = "require \"variables\"; set \"state\" \"pending\"; if string :is \" ${state} \" \" pending \" { keep; } else { discard; }";
    let script = parse(source).expect("valid string test");
    let (plan, _) = execute_state(&script, &[], 0, "", "").expect("execute string test");

    assert!(plan.keep);
    assert!(!plan.discard);
}

#[test]
fn string_matches_updates_captures_and_short_circuits_anyof() {
    let script = parse(
        "require \"variables\"; if anyof (true, string :matches \"invoice-42\" \"invoice-*\") { set \"capture\" \"${1}\"; }",
    )
    .expect("valid short-circuit script");
    let (_, variables) = execute_state(&script, &[], 0, "", "").expect("execute test");
    assert_eq!(variables.get("capture"), Some(""));

    let script = parse(
        "require \"variables\"; if string :matches \"invoice-42\" \"invoice-*\" { set \"capture\" \"${1}\"; }",
    )
    .expect("valid string capture script");
    let (_, variables) = execute_state(&script, &[], 0, "", "").expect("execute capture");
    assert_eq!(variables.get("capture"), Some("42"));
}

#[test]
fn string_test_rejects_referenced_non_utf8_octet_capture() {
    let script = parse(
        "require \"variables\"; if header :matches \"Subject\" \"??\" { if string :is \"${1}\" \"x\" { keep; } }",
    )
    .expect("valid octet script");
    let headers = vec![("Subject".to_owned(), "é".to_owned())];

    assert_eq!(
        execute_state(&script, &headers, 0, "", "").map(|_| ()),
        Err(ExecutionError::VariableRuntime)
    );
}

#[test]
fn variable_runtime_error_uses_a_recoverable_durable_code() {
    assert_eq!(
        ExecutionError::from_code("variables-runtime-error"),
        Some(ExecutionError::VariableRuntime)
    );
}

#[test]
fn header_exists_address_and_envelope_expand_arguments_at_test_time() {
    let script = parse(
        "require [\"variables\", \"envelope\"]; set \"header_name\" \"Subject\"; set \"wanted\" \"Invoice\"; set \"recipient_field\" \"To\"; set \"recipient\" \"alice@example.test\"; if header :is \"${header_name}\" \"${wanted}\" { if exists \"${header_name}\" { if address :is \"${recipient_field}\" \"${recipient}\" { if envelope :is \"to\" \"${recipient}\" { keep; } } } } else { discard; }",
    )
    .expect("valid expanded-condition script");
    let headers = vec![
        ("Subject".to_owned(), "Invoice".to_owned()),
        ("To".to_owned(), "Alice <alice@example.test>".to_owned()),
    ];
    let (plan, _) = execute_state(
        &script,
        &headers,
        0,
        "sender@example.test",
        "alice@example.test",
    )
    .expect("execute expanded tests");

    assert!(plan.keep);
    assert!(!plan.discard);
}

#[test]
fn ordinary_conditions_do_not_expand_without_variables_requirement() {
    let script = parse("if header :is \"${field}\" \"Invoice\" { discard; }")
        .expect("base Sieve string remains literal");
    let headers = vec![("${field}".to_owned(), "Invoice".to_owned())];
    let (plan, _) = execute_state(&script, &headers, 0, "", "").expect("execute base test");

    assert!(plan.discard);
}

#[test]
fn fileinto_expands_target_and_revalidates_mailbox_segments() {
    let script = parse(
        "require [\"variables\", \"fileinto\"]; set \"folder\" \"Invoices/2026\"; fileinto \"${folder}\";",
    )
    .expect("valid expanded fileinto target");
    let (plan, _) = execute_state(&script, &[], 0, "", "").expect("execute fileinto");
    assert_eq!(plan.folders, vec!["Invoices/2026"]);

    let unsafe_script = parse(
        "require [\"variables\", \"fileinto\"]; set \"folder\" \"../outside\"; fileinto \"${folder}\";",
    )
    .expect("static template contains no traversal segment");
    assert_eq!(
        execute_state(&unsafe_script, &[], 0, "", "").map(|_| ()),
        Err(ExecutionError::VariableRuntime)
    );
}

#[test]
fn redirect_expands_and_revalidates_one_mailbox_before_planning() {
    let script = parse(
        "require \"variables\"; set \"recipient\" \"alice@example.test\"; redirect \"${recipient}\";",
    )
    .expect("valid dynamic redirect template");
    let (plan, _) = execute_state(&script, &[], 0, "", "").expect("valid address");
    assert_eq!(plan.redirects, vec!["alice@example.test"]);

    let unsafe_script = parse(
        "require \"variables\"; set \"recipient\" \"alice@example.test, attacker@example.test\"; redirect \"${recipient}\";",
    )
    .expect("address validation is deferred until interpolation");
    assert_eq!(
        execute_state(&unsafe_script, &[], 0, "", "").map(|_| ()),
        Err(ExecutionError::VariableRuntime)
    );
}

#[test]
fn declared_variables_namespace_has_empty_unknown_values() {
    let script = parse("require \"variables\"; set \"result\" \"${variables.unknown}\";")
        .expect("the Variables namespace is declared");
    let (_, values) = execute_state(&script, &[], 0, "", "").expect("expand namespace");

    assert_eq!(values.get("result"), Some(""));
}

#[test]
fn sieve_quoting_precedes_single_pass_variable_expansion() {
    let source = r#"require "variables"; set "foo" "ACME"; set "quoted" "\${foo}"; set "escaped" "\\${foo}"; set "dollar" "$"; set "literal" "${dollar}{beep}";"#;
    let script = parse(source).expect("quoted variable references");
    let (_, values) = execute_state(&script, &[], 0, "", "").expect("execute string actions");

    assert_eq!(values.get("quoted"), Some("ACME"));
    assert_eq!(values.get("escaped"), Some(r#"\ACME"#));
    assert_eq!(values.get("literal"), Some("${beep}"));
}

#[test]
fn comparator_selector_is_expanded_before_string_matching() {
    let source = r#"require "variables"; set "cmp" "i;octet"; if string :comparator "${cmp}" :is "a" "A" { keep; } else { discard; }"#;
    let script = parse(source).expect("dynamic comparator selector");
    let (plan, _) = execute_state(&script, &[], 0, "", "").expect("resolve comparator selector");

    assert!(plan.discard);
}

#[test]
fn unsupported_expanded_comparator_fails_the_sieve_evaluation() {
    let source = r#"require "variables"; set "cmp" "i;unknown"; if string :comparator "${cmp}" :is "a" "a" { keep; }"#;
    let script = parse(source).expect("deferred comparator selector");

    assert!(matches!(
        execute_state(&script, &[], 0, "", ""),
        Err(ExecutionError::VariableRuntime)
    ));
}

#[test]
fn address_comparator_selector_is_expanded_at_condition_time() {
    let source = r#"require "variables"; set "cmp" "i;octet"; if address :comparator "${cmp}" :is "From" "a@example.com" { keep; } else { discard; }"#;
    let script = parse(source).expect("dynamic address comparator");
    let headers = vec![("From".to_owned(), "A@example.com".to_owned())];
    let (plan, _) =
        execute_state(&script, &headers, 0, "", "").expect("resolve address comparator");

    assert!(plan.discard);
}
