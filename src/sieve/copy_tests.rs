//! Проверки RFC 3894 без транспортного или файлового слоя.

use super::*;

/// `:copy` добавляет действие, не отменяя implicit INBOX keep.
#[test]
fn copy_actions_preserve_implicit_keep_and_require_the_extension() {
    let fileinto = parse("require [\"fileinto\", \"copy\"]; fileinto :copy \"Invoices\";")
        .expect("fileinto copy parse");
    let fileinto_plan = execute(&fileinto, &[], 0).expect("fileinto copy execute");
    assert!(fileinto_plan.keep);
    assert_eq!(fileinto_plan.folders, vec!["Invoices"]);

    let redirect = parse("require \"copy\"; redirect :copy \"remote@peer.test\";")
        .expect("redirect copy parse");
    let redirect_plan = execute(&redirect, &[], 0).expect("redirect copy execute");
    assert!(redirect_plan.keep);
    assert_eq!(redirect_plan.redirects, vec!["remote@peer.test"]);
    assert!(parse("require \"fileinto\"; fileinto :copy \"Invoices\";").is_err());
    assert!(parse("redirect :copy \"remote@peer.test\";").is_err());
}

/// RFC 5228 требует нечувствительности tagged arguments к ASCII-регистру.
#[test]
fn copy_tag_is_ascii_case_insensitive_for_both_actions() {
    let fileinto = parse("require [\"fileinto\", \"copy\"]; fileinto :CoPy \"Archive\";")
        .expect("mixed-case fileinto copy parses");
    let redirect = parse("require \"copy\"; redirect :COPY \"remote@peer.test\";")
        .expect("uppercase redirect copy parses");

    assert_eq!(
        execute(&fileinto, &[], 0).expect("execute").folders,
        vec!["Archive"]
    );
    assert_eq!(
        execute(&redirect, &[], 0).expect("execute").redirects,
        vec!["remote@peer.test"]
    );
}

/// Следующий `discard` по-прежнему отменяет implicit keep после копирования.
#[test]
fn fileinto_copy_does_not_change_following_rule_semantics() {
    let source =
        "require [\"fileinto\", \"copy\"]; fileinto :copy \"Archive\"; if true { discard; }";
    let plan = execute(&parse(source).expect("parse"), &[], 0).expect("execute");
    assert!(!plan.keep);
    assert!(plan.discard);
    assert_eq!(plan.folders, vec!["Archive"]);
}
