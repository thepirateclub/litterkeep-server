//! Проверки типизированного синтаксиса SMTP DSN RFC 3461.

use super::*;

/// Регистр ключевых слов не меняет выбранные условия уведомления.
#[test]
fn notify_conditions_are_case_insensitive_and_unique() {
    let policy = parse_notify("success,Failure,DELAY").expect("valid notify list");
    let NotifyPolicy::Conditions(conditions) = policy else {
        panic!("expected a set of conditions");
    };
    assert_eq!(conditions.len(), 3);
    assert!(conditions.contains(&NotifyCondition::Success));
    assert!(conditions.contains(&NotifyCondition::Failure));
    assert!(conditions.contains(&NotifyCondition::Delay));
}

/// NEVER принимается отдельно, а пустые и повторяющиеся списки запрещены.
#[test]
fn notify_never_is_exclusive_and_lists_must_be_nonempty_and_unique() {
    assert_eq!(parse_notify("never"), Ok(NotifyPolicy::Never));
    assert_eq!(parse_notify(""), Err(ParseError::Syntax));
    assert_eq!(parse_notify("SUCCESS,"), Err(ParseError::Syntax));
    assert_eq!(parse_notify("SUCCESS,SUCCESS"), Err(ParseError::Syntax));
    assert_eq!(parse_notify("NEVER,FAILURE"), Err(ParseError::Syntax));
}

/// Xtext принимает незакодированный безопасный ASCII и uppercase escapes.
#[test]
fn xtext_decodes_safe_octets_and_preserves_ascii_space() {
    assert_eq!(
        decode_xtext("alice+20example", 100),
        Ok(b"alice example".to_vec())
    );
    assert_eq!(decode_xtext("a+2Bb", 100), Ok(b"a+b".to_vec()));
}

/// Xtext отклоняет запрещённые разделители, строчные escape и неверный hex.
#[test]
fn xtext_rejects_invalid_or_non_printable_values() {
    for invalid in ["", "a=b", "a+b", "a+2b", "a+GG", "a+00", "é"] {
        assert_eq!(
            decode_xtext(invalid, 100),
            Err(ParseError::Syntax),
            "{invalid}"
        );
    }
    assert_eq!(decode_xtext("abcd", 3), Err(ParseError::Syntax));
}
