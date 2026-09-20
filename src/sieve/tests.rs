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
    let script = parse("# note\nif false { discard; } else { keep; }").expect("parse");
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
        &parse("keep; redirect \"remote@peer.test\"; fileinto \"Archive\";")
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
        "if false { discard; } elsif true { fileinto \"Invoices\"; stop; } else { discard; }",
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
        "if allof [header :matches :comparator \"i;ascii-casemap\" [\"subject\", \"x-topic\"] [\"invoice-*\"], not exists \"x-cancel\", size :over 1K] { fileinto \"Invoices\"; } else { discard; }",
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

/// Address-проекция отделяет plus-detail и использует тот же typed matcher.
#[test]
fn address_test_selects_detail_without_matching_another_recipient() {
    let script = parse(
        "if address :detail :is \"to\" \"sales\" { fileinto \"Invoices\"; } else { discard; }",
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

/// Сетевой fixture использует это составное правило без подмены envelope RCPT.
#[test]
fn combined_header_address_and_size_rule_matches_network_fixture() {
    let script = parse(
        "require [\"fileinto\"]; if allof [header :contains \"subject\" \"[invoice]\", address :detail :is \"to\" \"invoices\", size :over 1] { fileinto \"Invoices\"; stop; }",
    )
    .expect("parse");
    let plan = execute(
        &script,
        &[
            ("Subject".into(), "[invoice] paid".into()),
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
    assert!(parse("fileinto \"../Invoices\";").is_err());
    assert!(
        parse("if header :comparator \"i;unicode-casemap\" \"subject\" \"x\" { keep; }").is_err()
    );
    assert!(parse("if size :over 1T { keep; }").is_err());
}
