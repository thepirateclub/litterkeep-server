//! Проверки Sieve match-variable state и порядка вычисления условий.

use super::*;
use crate::sieve::ast::{Comparator, Condition, MatchType, StringMatch};

fn context<'a>(headers: &'a [(String, String)]) -> MessageContext<'a> {
    MessageContext {
        headers,
        size: 0,
        envelope_from: "",
        envelope_to: "",
    }
}

fn header_match() -> Condition {
    Condition::Header {
        names: vec!["Subject".to_owned()],
        matcher: StringMatch {
            match_type: MatchType::Matches,
            comparator: Comparator::AsciiCasemap,
            keys: vec!["[*] *".to_owned()],
        },
    }
}

#[test]
fn successful_header_matches_replace_match_variables() {
    let headers = vec![("Subject".to_owned(), "[Invoices] update".to_owned())];
    let mut variables = MatchVariables::default();

    assert!(matches(&header_match(), &context(&headers), &mut variables));
    assert_eq!(variables.get(0), Some(&b"[Invoices] update"[..]));
    assert_eq!(variables.get(1), Some(&b"Invoices"[..]));
    assert_eq!(variables.get(2), Some(&b"update"[..]));
    assert_eq!(variables.get(3), None);
}

#[test]
fn anyof_does_not_evaluate_or_capture_from_skipped_tests() {
    let headers = vec![("Subject".to_owned(), "[Invoices] update".to_owned())];
    let condition = Condition::AnyOf(vec![Condition::True, header_match()]);
    let mut variables = MatchVariables::default();

    assert!(matches(&condition, &context(&headers), &mut variables));
    assert_eq!(variables.get(0), None);
}

#[test]
fn failed_match_does_not_replace_previous_captures() {
    let headers = vec![("Subject".to_owned(), "[Invoices] update".to_owned())];
    let mut variables = MatchVariables::default();
    assert!(matches(&header_match(), &context(&headers), &mut variables));
    let previous = variables.get(1).map(ToOwned::to_owned);
    let failed = Condition::Header {
        names: vec!["Subject".to_owned()],
        matcher: StringMatch {
            match_type: MatchType::Matches,
            comparator: Comparator::Octet,
            keys: vec!["other*".to_owned()],
        },
    };

    assert!(!matches(&failed, &context(&headers), &mut variables));
    assert_eq!(variables.get(1), previous.as_deref());
}
