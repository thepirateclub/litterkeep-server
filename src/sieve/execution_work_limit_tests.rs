//! Проверяет, что исчерпание matcher budget доходит до delivery runtime как ошибка.

use super::*;
use crate::sieve::ast::{Comparator, Condition, MatchType, StringMatch};

#[test]
fn glob_work_limit_is_not_misreported_as_a_sieve_non_match() {
    let matcher = StringMatch {
        match_type: MatchType::Matches,
        comparator: Comparator::Octet,
        keys: vec!["*a".repeat(1_100).into_bytes()],
    };
    let script = Script {
        requires: Vec::new(),
        commands: vec![Command::If {
            condition: Condition::Header {
                names: vec!["Subject".to_owned()],
                matcher,
            },
            then_commands: Vec::new(),
            else_commands: Vec::new(),
        }],
    };
    let headers = vec![("Subject".to_owned(), format!("{}b", "a".repeat(1_100)))];

    assert_eq!(
        execute(&script, &headers, 0),
        Err(ExecutionError::MatcherLimit)
    );
    assert_eq!(
        ExecutionError::from_code("matcher-work-limit"),
        Some(ExecutionError::MatcherLimit)
    );
}
