//! Unit-проверки grammar и plan, не использующие delivery implementation.

use super::*;

#[test]
fn invoice_rule_creates_only_invoices_delivery() {
    let script = parse("require [\"fileinto\"]; if header :contains \"subject\" \"[invoice]\" { fileinto \"Invoices\"; stop; }").expect("parse");
    let plan =
        execute(&script, &[("Subject".into(), "[invoice] September".into())]).expect("execute");
    assert_eq!(plan.folders, vec!["Invoices"]);
    assert!(!plan.keep);
}

#[test]
fn comments_and_implicit_keep_survive_false_condition() {
    let script = parse("# note\nif false { discard; } else { keep; }").expect("parse");
    let plan = execute(&script, &[]).expect("execute");
    assert!(plan.keep);
    assert!(!plan.discard);
}

#[test]
fn invalid_script_does_not_become_a_delivery_plan() {
    assert!(parse("require [\"vacation\"]; keep;").is_err());
    assert!(parse("fileinto \"../Invoices\";").is_err());
}
