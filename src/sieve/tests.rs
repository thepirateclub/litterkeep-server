//! Unit-проверки grammar и plan, не использующие delivery implementation.

use super::*;

#[test]
fn invoice_rule_creates_only_invoices_delivery() {
    let script = parse("require [\"fileinto\"]; if header :contains \"subject\" \"[invoice]\" { fileinto \"Invoices\"; stop; }").expect("parse");
    let plan = execute(
        &script,
        &[("Subject".into(), "[invoice] September".into())],
        100,
    )
    .expect("execute");
    assert_eq!(plan.folders, vec!["Invoices"]);
    assert!(!plan.keep);
}

#[test]
fn comments_and_implicit_keep_survive_false_condition() {
    let script = parse("# note\r\nif false { discard; } else { keep; }").expect("parse");
    let plan = execute(&script, &[], 0).expect("execute");
    assert!(plan.keep);
    assert!(!plan.discard);
}

/// Redirect сам отменяет implicit keep, но не явное действие keep.
#[test]
fn redirect_and_fileinto_preserve_only_explicit_keep() {
    let redirect = execute(
        &parse("redirect \"remote@peer.test\";").expect("redirect parse"),
        &[],
        0,
    )
    .expect("redirect execute");
    assert!(!redirect.keep);
    assert_eq!(redirect.redirects, vec!["remote@peer.test"]);

    let combined = execute(
        &parse("require \"fileinto\"; keep; redirect \"remote@peer.test\"; fileinto \"Archive\";")
            .expect("combined parse"),
        &[],
        0,
    )
    .expect("combined execute");
    assert!(combined.keep);
    assert_eq!(combined.redirects, vec!["remote@peer.test"]);
    assert_eq!(combined.folders, vec!["Archive"]);
}

/// `elsif` выбирает первую совпавшую ветвь и не исполняет следующий else.
#[test]
fn elsif_is_an_exclusive_bounded_branch() {
    let script = parse(
        "require \"fileinto\"; if false { discard; } elsif true { fileinto \"Invoices\"; stop; } else { discard; }",
    )
    .expect("parse");
    let plan = execute(&script, &[], 0).expect("execute");
    assert_eq!(plan.folders, vec!["Invoices"]);
    assert!(!plan.discard);
}

/// Header списки, glob, comparator и size получают один bounded DeliveryPlan.
#[test]
fn header_matchers_and_size_select_the_expected_branch() {
    let script = parse(
        "require \"fileinto\"; if allof (header :matches :comparator \"i;ascii-casemap\" [\"subject\", \"x-topic\"] [\"invoice-*\"], not exists \"x-cancel\", size :over 1K) { fileinto \"Invoices\"; } else { discard; }",
    )
    .expect("parse");
    let plan =
        execute(&script, &[("Subject".into(), "Invoice-2026".into())], 1_025).expect("execute");
    assert_eq!(plan.folders, vec!["Invoices"]);
    assert!(!plan.discard);
}

/// Octet comparator не подменяет явно выбранный регистр ASCII casemap-ом.
#[test]
fn octet_comparator_keeps_case_significant() {
    let script =
        parse("if header :is :comparator \"i;octet\" \"subject\" \"Invoice\" { discard; }")
            .expect("parse");
    let plan = execute(&script, &[("Subject".into(), "invoice".into())], 10).expect("execute");
    assert!(plan.keep);
}

#[test]
fn matches_uses_octet_width_and_sieve_escaped_wildcards() {
    let one = parse("if header :matches :comparator \"i;octet\" \"subject\" \"?\" { discard; }")
        .expect("parse one octet wildcard");
    let plan = execute(&one, &[("Subject".into(), "é".into())], 10).expect("execute");
    assert!(plan.keep);

    let two = parse("if header :matches :comparator \"i;octet\" \"subject\" \"??\" { discard; }")
        .expect("parse two octet wildcards");
    let plan = execute(&two, &[("Subject".into(), "é".into())], 10).expect("execute");
    assert!(plan.discard);

    let escaped = parse(r#"if header :matches "subject" "invoice\\*" { discard; }"#)
        .expect("parse escaped wildcard");
    let plan = execute(&escaped, &[("Subject".into(), "invoice*".into())], 10).expect("execute");
    assert!(plan.discard);
}

#[test]
fn header_test_decodes_rfc2047_charsets_before_comparison() {
    let script = parse("if header :is \"subject\" \"café☃\" { discard; }")
        .expect("parse Unicode header matcher");
    let encoded = "=?ISO-8859-1?Q?caf=E9?= =?UTF-8?B?4piD?=";
    let plan = execute(&script, &[("Subject".into(), encoded.into())], 10).expect("execute");
    assert!(plan.discard);
}

#[test]
fn header_test_ignores_leading_and_trailing_space_and_tab() {
    let script = parse("if header :is \"subject\" \"invoice\" { discard; }")
        .expect("parse whitespace matcher");
    let plan =
        execute(&script, &[("Subject".into(), " \tinvoice\t ".into())], 10).expect("execute");
    assert!(plan.discard);
}

#[test]
fn header_test_accepts_empty_keys_and_invalid_header_names() {
    let empty_contains = parse("if header :contains \"Subject\" \"\" { discard; }")
        .expect("empty contains key is valid");
    let plan = execute(&empty_contains, &[("Subject".into(), "invoice".into())], 10)
        .expect("execute empty contains key");
    assert!(plan.discard);

    let absent = execute(&empty_contains, &[], 10).expect("execute absent header");
    assert!(!absent.discard);

    let empty_is =
        parse("if header :is \"Subject\" \"\" { discard; }").expect("empty is key is valid");
    let plan = execute(&empty_is, &[("Subject".into(), "invoice".into())], 10)
        .expect("execute empty is key");
    assert!(!plan.discard);

    let invalid_name = parse("if header :is \"\" \"invoice\" { discard; }")
        .expect("invalid header name is not a script error");
    let plan = execute(&invalid_name, &[("Subject".into(), "invoice".into())], 10)
        .expect("execute invalid header name");
    assert!(!plan.discard);
}

/// Address-проекция отделяет plus-detail и использует тот же typed matcher.
#[test]
fn address_test_selects_detail_without_matching_another_recipient() {
    let script = parse(
        "require [\"subaddress\", \"fileinto\"]; if address :detail :is \"to\" \"sales\" { fileinto \"Invoices\"; } else { discard; }",
    )
    .expect("parse");
    let plan = execute(
        &script,
        &[(
            "To".into(),
            "\"Alice, Example\" <alice+sales@example.test>, bob@example.test".into(),
        )],
        20,
    )
    .expect("execute");
    assert_eq!(plan.folders, vec!["Invoices"]);
}

/// RFC 5228 разрешает менять порядок tagged arguments до string-list.
#[test]
fn address_options_accept_arbitrary_tag_order_and_keep_comparator_semantics() {
    let source = "require \"subaddress\"; if address :comparator \"i;octet\" :detail :contains \"to\" \"SALES\" { discard; }";
    let message = [("To".into(), "alice+sales@example.test".into())];
    let case_sensitive = parse(source).expect("reordered address options parse");
    let plan = execute(&case_sensitive, &message, 10).expect("execute");
    assert!(plan.keep);

    let matching_key = parse(&source.replace("SALES", "sales")).expect("parse");
    let plan = execute(&matching_key, &message, 10).expect("execute");
    assert!(plan.discard);
}

#[test]
fn address_test_ignores_unstructured_fields_and_matches_group_members() {
    let script = parse("if address :is [\"Subject\", \"To\"] \"bob@example.test\" { discard; }")
        .expect("parse");
    let headers = [
        ("Subject".into(), "bob@example.test".into()),
        (
            "To".into(),
            "Billing: alice@example.test, bob@example.test;".into(),
        ),
    ];
    let plan = execute(&script, &headers, 10).expect("execute");
    assert!(plan.discard);

    let group_name = parse("if address :is \"to\" \"Billing\" { discard; }").expect("parse");
    let plan = execute(&group_name, &headers, 10).expect("execute");
    assert!(plan.keep);
}

#[test]
fn address_test_includes_resent_address_fields() {
    let script =
        parse("if address :is \"resent-to\" \"bob@example.test\" { discard; }").expect("parse");
    let plan = execute(
        &script,
        &[("Resent-To".into(), "bob@example.test".into())],
        10,
    )
    .expect("execute");
    assert!(plan.discard);
}

/// Сетевой fixture использует это составное правило без подмены envelope RCPT.
#[test]
fn combined_header_address_and_size_rule_matches_network_fixture() {
    let script = parse(
        r#"require ["fileinto", "subaddress"]; if allof (header :contains "subject" "[invoice]", header :matches "subject" "invoice\\**", address :detail :is "to" "invoices", size :over 1) { fileinto "Invoices"; stop; }"#,
    )
    .expect("parse");
    let plan = execute(
        &script,
        &[
            ("Subject".into(), "invoice* [invoice] paid".into()),
            ("To".into(), "bob+invoices@litterkeep.test".into()),
        ],
        100,
    )
    .expect("execute");
    assert_eq!(plan.folders, vec!["Invoices"]);
}

#[test]
fn invalid_script_does_not_become_a_delivery_plan() {
    assert!(parse("require [\"vacation\"]; keep;").is_err());
    assert!(parse("fileinto \"Invoices\";").is_err());
    assert!(parse("fileinto \"../Invoices\";").is_err());
    assert!(
        parse("if header :comparator \"i;unicode-casemap\" \"subject\" \"x\" { keep; }").is_err()
    );
    assert!(parse("if size :over 1T { keep; }").is_err());
    assert!(parse("if allof [true, false] { keep; }").is_err());
    assert!(parse("if allof () { keep; }").is_err());
    assert!(parse("if allof (true false) { keep; }").is_err());
    assert!(parse("if anyof (true,) { keep; }").is_err());
    assert!(parse("if exists [] { keep; }").is_err());
    assert!(parse("if exists [\"Subject\" \"From\"] { keep; }").is_err());
    assert!(parse("if exists [\"Subject\",] { keep; }").is_err());
    assert!(parse("if exists [,\"Subject\"] { keep; }").is_err());
    assert!(parse("if address :detail :is \"to\" \"sales\" { keep; }").is_err());
    assert!(parse("if address :user :is \"to\" \"alice\" { keep; }").is_err());
    assert!(parse("if address :detail :localpart \"to\" \"alice\" { keep; }").is_err());
    assert!(parse("if address :is :contains \"to\" \"alice\" { keep; }").is_err());
}

#[test]
fn subaddress_projection_requires_and_accepts_its_capability() {
    assert!(
        parse("require \"subaddress\"; if address :user :is \"to\" \"alice\" { keep; }").is_ok()
    );
}

/// RFC 5228 test-list использует круглые скобки, в отличие от string-list.
#[test]
fn allof_and_anyof_parse_parenthesized_comma_separated_tests() {
    let script = parse("if anyof (false, allof (true, true)) { keep; }").expect("parse");
    let plan = execute(&script, &[], 0).expect("execute");
    assert!(plan.keep);
}

/// String-list требует непустые строки с одним comma между соседями.
#[test]
fn string_list_requires_nonempty_comma_separated_values() {
    let script = parse("if exists [\"Subject\", \"From\"] { keep; }").expect("parse");
    assert!(execute(&script, &[("Subject".into(), "x".into())], 0).is_ok());
}

/// Несколько require допустимы подряд, но после команды расширения запрещены.
#[test]
fn require_declarations_must_precede_every_command() {
    assert!(parse("require \"fileinto\"; require [\"fileinto\"]; keep;").is_ok());
    assert!(parse("keep; require \"fileinto\";").is_err());
    assert!(parse("if true { keep; } require \"fileinto\";").is_err());
}
