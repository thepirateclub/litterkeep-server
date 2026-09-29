//! Проверки store, expansion и bounded-правил RFC 5229 Variables.

use super::{
    MAX_VARIABLE_VALUE_CHARS, MAX_VARIABLES, VariableError, VariableModifier, VariableStore,
};

#[test]
fn names_are_ascii_case_insensitive_and_scoped_to_store() {
    let mut first = VariableStore::default();
    let second = VariableStore::default();
    first.set("Folder", "Invoices").expect("valid name");

    assert_eq!(first.get("FOLDER"), Some("Invoices"));
    assert_eq!(second.get("folder"), None);
}

#[test]
fn expansion_is_single_pass_and_unknown_reference_is_empty() {
    let mut store = VariableStore::default();
    store.set("word", "${missing}").expect("valid name");

    assert_eq!(
        store.expand("${WORD}/${other}", &[]).expect("bounded"),
        "${missing}/"
    );
}

#[test]
fn malformed_reference_stays_literal_and_nested_valid_reference_expands() {
    let mut store = VariableStore::default();
    store.set("company", "ACME").expect("valid name");

    assert_eq!(
        store
            .expand("${doh!} ${foo.bar!} ${BAD${Company}", &[])
            .expect("bounded"),
        "${doh!} ${foo.bar!} ${BADACME"
    );
}

#[test]
fn numeric_references_allow_leading_zeroes_and_out_of_range_is_empty() {
    let store = VariableStore::default();
    let captures = vec!["whole".to_owned(), "list".to_owned()];

    assert_eq!(
        store
            .expand("${0}:${1}:${01}:${9}", &captures)
            .expect("bounded"),
        "whole:list:list:"
    );
    assert_eq!(
        store.expand("${10}:${999999999999999999999999}", &captures),
        Ok(":".to_owned())
    );
    assert_eq!(
        store.expand("${vendor.value}", &captures),
        Err(VariableError::UnsupportedNamespace)
    );
}

#[test]
fn expansion_rejects_output_above_the_explicit_runtime_budget() {
    let mut store = VariableStore::default();
    store
        .set("wide", &"x".repeat(MAX_VARIABLE_VALUE_CHARS))
        .expect("valid name");
    let source = "${wide}".repeat(17);

    assert_eq!(
        store.expand(&source, &[]),
        Err(VariableError::ExpansionLimit)
    );
}

#[test]
fn store_enforces_name_and_count_limits_and_truncates_by_unicode_character() {
    let mut store = VariableStore::default();
    assert_eq!(store.set("9bad", "x"), Err(VariableError::InvalidName));
    let long_value = "я".repeat(MAX_VARIABLE_VALUE_CHARS + 3);
    store.set("v0", &long_value).expect("valid name");
    assert_eq!(
        store.get("V0").map(str::chars).map(Iterator::count),
        Some(4_000)
    );
    for number in 1..MAX_VARIABLES {
        store
            .set(&format!("v{number}"), "x")
            .expect("within variable limit");
    }
    assert_eq!(
        store.set("overflow", "x"),
        Err(VariableError::TooManyVariables)
    );
}

#[test]
fn set_modified_applies_case_and_unicode_length_before_storage() {
    let mut store = VariableStore::default();
    store
        .set_modified(
            "summary",
            "Jumbled É",
            &[VariableModifier::Lower, VariableModifier::Length],
        )
        .expect("valid variable and modifiers");

    assert_eq!(store.get("SUMMARY"), Some("9"));
}

/// RFC 5229 раскрывает текущее значение до применения `set` modifiers.
#[test]
fn set_expanded_uses_current_values_captures_then_modifies_once() {
    let mut store = VariableStore::default();
    store
        .set("greeting", "juMBlEd lETteRS")
        .expect("valid name");
    let captures = vec!["unused".to_owned(), "*".to_owned()];

    store
        .set_expanded(
            "result",
            "${greeting} ${1}",
            &captures,
            &[
                VariableModifier::UpperFirst,
                VariableModifier::Lower,
                VariableModifier::QuoteWildcard,
            ],
        )
        .expect("expand before modifiers");

    assert_eq!(store.get("result"), Some("Jumbled letters \\*"));
}
