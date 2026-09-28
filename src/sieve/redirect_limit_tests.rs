//! Проверяет общий redirect budget, публикуемый ManageSieve capability.

use super::{ExecutionError, MAX_REDIRECTS_PER_EVALUATION, execute, parse};

#[derive(Clone, Copy)]
enum RedirectMode {
    Standard,
    Copy,
}

/// Публикуемый лимит действий принимается, а следующий redirect отклоняется.
#[test]
fn redirect_limit_matches_exactly_sixteen_actions_per_evaluation() {
    for mode in [RedirectMode::Standard, RedirectMode::Copy] {
        let within_limit = script(mode, MAX_REDIRECTS_PER_EVALUATION);
        let parsed = parse(&within_limit).expect("sixteen redirect actions parse");
        let plan = execute(&parsed, &[], 0).expect("sixteen redirects are allowed");
        assert_eq!(plan.redirects.len(), MAX_REDIRECTS_PER_EVALUATION);

        let beyond_limit = script(mode, MAX_REDIRECTS_PER_EVALUATION + 1);
        let parsed = parse(&beyond_limit).expect("seventeen redirect actions parse");
        assert!(matches!(
            execute(&parsed, &[], 0),
            Err(ExecutionError::RedirectLimit)
        ));
    }
}

/// Строит bounded script с заданным числом отдельных redirect actions.
fn script(mode: RedirectMode, count: usize) -> String {
    let (preamble, action) = match mode {
        RedirectMode::Standard => ("", "redirect \"user@example.test\";\r\n"),
        RedirectMode::Copy => (
            "require \"copy\";\r\n",
            "redirect :copy \"user@example.test\";\r\n",
        ),
    };
    let mut source = preamble.to_owned();
    source.push_str(&std::iter::repeat_n(action, count).collect::<String>());
    source
}
