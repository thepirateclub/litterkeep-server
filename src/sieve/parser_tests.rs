//! RFC 5228 parser rejection and list grammar regressions.

use super::*;

/// Некорректные команды, options и списки не становятся DeliveryPlan.
#[test]
fn invalid_script_does_not_become_a_delivery_plan() {
    for source in [
        "require [\"unknown-extension\"]; keep;",
        "fileinto \"Invoices\";",
        "fileinto \"../Invoices\";",
        "if header :comparator \"i;unicode-casemap\" \"subject\" \"x\" { keep; }",
        "if size :over 1T { keep; }",
        "if allof [true, false] { keep; }",
        "if allof () { keep; }",
        "if allof (true false) { keep; }",
        "if anyof (true,) { keep; }",
        "if exists [] { keep; }",
        "if exists [\"Subject\" \"From\"] { keep; }",
        "if exists [\"Subject\",] { keep; }",
        "if exists [,\"Subject\"] { keep; }",
        "if address :detail :is \"to\" \"sales\" { keep; }",
        "if address :user :is \"to\" \"alice\" { keep; }",
        "if address :detail :localpart \"to\" \"alice\" { keep; }",
        "if address :is :contains \"to\" \"alice\" { keep; }",
    ] {
        assert!(parse(source).is_err(), "accepted invalid script: {source}");
    }
}

/// Неизвестные или ещё не реализованные расширения не принимаются молча.
#[test]
fn selected_unimplemented_extension_requirements_fail_closed() {
    for source in [
        "require \"body\"; keep;",
        "require \"reject\"; keep;",
        "require \"imap4flags\"; keep;",
        "require \"mailboxid\"; keep;",
        "require \"mailbox\"; keep;",
        "require \"special-use\"; keep;",
    ] {
        assert!(
            parse(source).is_err(),
            "accepted unsupported extension: {source}"
        );
    }
}

/// RFC 5229 требует Variables для `set` и запрещает некорректные modifiers.
#[test]
fn variables_set_requires_capability_and_valid_constant_name() {
    assert!(parse("set \"name\" \"value\";").is_err());
    assert!(parse("require \"variables\"; set \"9bad\" \"value\";").is_err());
    assert!(parse("require \"variables\"; set :vendor \"name\" \"value\";").is_err());
    assert!(parse("require \"variables\"; set :upper :lower \"name\" \"value\";").is_err());
    assert!(parse("require \"variables\"; set :LeNgTh \"name\" \"value\";").is_ok());
}

#[test]
fn variables_string_test_requires_extension_and_accepts_list_arguments() {
    assert!(parse("if string \"source\" \"source\" { keep; }").is_err());
    let script = parse(
        "require \"variables\"; if string :matches [\" source \", \"other\"] [\"*source*\", \"none\"] { keep; }",
    );
    assert!(script.is_ok());
}

#[test]
fn variable_namespace_references_are_checked_when_the_script_is_loaded() {
    assert!(parse("require \"variables\"; set \"name\" \"${environment.domain}\";").is_err());
    assert!(parse("require \"variables\"; set \"name\" \"${variables.unknown}\";").is_ok());
    assert!(
        parse("require \"variables\"; if header :is \"${environment.field}\" \"x\" { keep; }")
            .is_err()
    );
    assert!(
        parse("require \"variables\"; if false { set \"value\" \"${environment.field}\"; }")
            .is_err()
    );
    assert!(parse("if header :is \"${environment.field}\" \"x\" { keep; }").is_ok());
}

#[test]
fn dynamic_comparator_requires_variables_and_valid_namespace() {
    assert!(parse("if string :comparator \"${cmp}\" \"x\" \"x\" { keep; }").is_err());
    assert!(
        parse("require \"variables\"; if string :comparator \"${cmp}\" \"x\" \"x\" { keep; }")
            .is_ok()
    );
    assert!(parse("require \"variables\"; if string :comparator \"${environment.cmp}\" \"x\" \"x\" { keep; }").is_err());
}

#[test]
fn dynamic_envelope_part_requires_variables_and_valid_namespace() {
    assert!(
        parse("require \"envelope\"; if envelope \"${part}\" \"sender@example.com\" { keep; }")
            .is_err()
    );
    assert!(parse("require [\"envelope\", \"variables\"]; if envelope \"${part}\" \"sender@example.com\" { keep; }").is_ok());
    assert!(parse("require [\"envelope\", \"variables\"]; if envelope \"${environment.part}\" \"sender@example.com\" { keep; }").is_err());
}

/// RFC 5228 требует объявления subaddress до использования address :user.
#[test]
fn subaddress_projection_requires_and_accepts_its_capability() {
    assert!(
        parse("require \"subaddress\"; if address :user :is \"to\" \"alice\" { keep; }").is_ok()
    );
}

/// RFC 5228 test-list использует круглые скобки и запятые.
#[test]
fn allof_and_anyof_parse_parenthesized_comma_separated_tests() {
    let script = parse("if anyof (false, allof (true, true)) { keep; }").expect("parse");
    let plan = execute(&script, &[], 0).expect("execute");
    assert!(plan.keep);
}

/// RFC 5228 string-list требует непустые строки между одиночными запятыми.
#[test]
fn string_list_requires_nonempty_comma_separated_values() {
    let script = parse("if exists [\"Subject\", \"From\"] { keep; }").expect("parse");
    assert!(execute(&script, &[("Subject".into(), "x".into())], 0).is_ok());
}

/// Parser переносит raw non-UTF-8 string octets в byte-valued matcher AST.
#[test]
fn parser_preserves_non_utf8_header_match_key_octets() {
    let script =
        crate::sieve::parse_octets(b"if header :contains \"X-Test\" \"A\xff\x80Z\" { keep; }")
            .expect("RFC 5228 string octets parse without text conversion");
    let Some(crate::sieve::ast::Command::If {
        condition: crate::sieve::Condition::Header { matcher, .. },
        ..
    }) = script.commands.first()
    else {
        panic!("header test must remain in the typed AST");
    };
    assert_eq!(matcher.keys.first(), Some(&b"A\xff\x80Z".to_vec()));
}

/// Require declarations остаются перед каждой командой расширения.
#[test]
fn require_declarations_must_precede_every_command() {
    assert!(parse("require \"fileinto\"; require [\"fileinto\"]; keep;").is_ok());
    assert!(parse("keep; require \"fileinto\";").is_err());
    assert!(parse("if true { keep; } require \"fileinto\";").is_err());
}

/// Проверяет ASCII-регистр у синтаксических токенов, но не у строковых данных.
#[test]
fn syntax_tokens_ignore_case_while_string_values_remain_exact() {
    let source = "REQUIRE \"fileinto\"; IF HEADER :IS :COMPARATOR \"i;ascii-casemap\" \"Subject\" \"Invoice\" { FILEINTO \"Invoices/2026\"; } eLsE { KEEP; }";
    let script = parse(source).expect("case-insensitive syntax");
    let plan = execute(&script, &[("Subject".into(), "Invoice".into())], 0)
        .expect("execute matching header");
    assert_eq!(plan.folders.as_slice(), &["Invoices/2026"]);
    assert!(parse("IF HEADER :IS \"subject\" \"invoice\" { KEEP; }").is_ok());
    assert!(parse("REQUIRE \"FILEINTO\"; FILEINTO \"Invoices\";").is_err());
}

/// Address tags и comparator names также являются case-insensitive tokens.
#[test]
fn address_tags_and_comparator_names_ignore_ascii_case() {
    let source =
        "IF ADDRESS :LOCALPART :IS :COMPARATOR \"I;ASCII-CASEMAP\" \"to\" \"alice\" { KEEP; }";
    let script = parse(source).expect("case-insensitive address options");
    let plan = execute_with_envelope(&script, &[], 0, "", "Alice@example.test");
    assert!(plan.expect("execute address test").keep);
}
