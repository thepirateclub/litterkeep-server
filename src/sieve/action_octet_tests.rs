//! Проверяет octet сохранение Sieve delivery action строк.

use super::{ExecutionError, execute, parse};

#[test]
fn fileinto_accepts_binary_string_syntax_then_fails_at_text_boundary() {
    let script = parse("require [\"fileinto\", \"encoded-character\"]; fileinto \"${hex:FF}\";")
        .expect("binary Sieve string is syntactically valid");
    assert_eq!(
        execute(&script, &[], 0),
        Err(ExecutionError::VariableRuntime)
    );
}

#[test]
fn parser_accepts_raw_non_utf8_fileinto_string_octets() {
    let script = super::parse_octets(b"require \"fileinto\"; fileinto \"\xff\";")
        .expect("RFC Sieve string octets");
    assert_eq!(
        execute(&script, &[], 0),
        Err(ExecutionError::VariableRuntime)
    );
}

#[test]
fn encoded_unicode_fileinto_value_reaches_mailbox_delivery_plan() {
    let script = parse(
        "require [\"fileinto\", \"encoded-character\"]; fileinto \"${unicode:49 6e 76 6f 69 63 65 73 2f 32 30 32 36}\";",
    )
    .expect("encoded Unicode folder");
    let plan = execute(&script, &[], 0).expect("delivery plan");
    assert_eq!(plan.folders, vec!["Invoices/2026"]);
}

#[test]
fn redirect_octets_are_validated_after_ast_parsing() {
    let script = parse("require \"encoded-character\"; redirect \"${hex:FF}\";")
        .expect("binary address string is syntactically valid");
    assert_eq!(
        execute(&script, &[], 0),
        Err(ExecutionError::VariableRuntime)
    );
}
