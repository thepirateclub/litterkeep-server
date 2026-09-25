//! Проверяет envelope test отдельно от MIME address headers и SMTP I/O.

use super::{execute_with_envelope, parse};

#[test]
fn envelope_test_requires_capability_and_matches_actual_smtp_paths() {
    assert!(parse("if envelope :is \"to\" \"bob@test\" { discard; }").is_err());
    let to_script =
        parse("require \"envelope\"; if envelope :is \"TO\" \"bob@litterkeep.test\" { discard; }")
            .expect("to script");
    let matched = execute_with_envelope(
        &to_script,
        &[],
        0,
        "alice@litterkeep.test",
        "bob@litterkeep.test",
    )
    .expect("to execution");
    assert!(matched.discard);
    let unmatched = execute_with_envelope(
        &to_script,
        &[],
        0,
        "alice@litterkeep.test",
        "other@litterkeep.test",
    )
    .expect("unmatched execution");
    assert!(!unmatched.discard);
}

#[test]
fn envelope_from_and_null_reverse_path_use_the_smtp_transaction() {
    let from_script = parse(
        "require \"envelope\"; if envelope :is \"from\" \"alice@litterkeep.test\" { discard; }",
    )
    .expect("from script");
    assert!(
        execute_with_envelope(
            &from_script,
            &[],
            0,
            "alice@litterkeep.test",
            "bob@litterkeep.test",
        )
        .expect("from execution")
        .discard
    );
    let null_script =
        parse("require \"envelope\"; if envelope :domain :is \"from\" \"\" { discard; }")
            .expect("null sender script");
    assert!(
        execute_with_envelope(&null_script, &[], 0, "", "bob@litterkeep.test")
            .expect("null sender execution")
            .discard
    );
}

#[test]
fn envelope_subaddress_projection_requires_its_capability() {
    assert!(parse("require \"envelope\"; if envelope :user :is \"to\" \"bob\" { keep; }").is_err());
    assert!(
        parse(
            "require [\"envelope\", \"subaddress\"]; if envelope :user :is \"to\" \"bob\" { keep; }"
        )
        .is_ok()
    );
    assert!(
        parse("require \"envelope\"; if envelope :is \"unknown\" \"value\" { keep; }").is_err()
    );
}
