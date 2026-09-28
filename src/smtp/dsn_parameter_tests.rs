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

/// RFC 6533 unitext escapes produce exact UTF-8 bytes for an ORCPT value.
#[test]
fn utf8_xtext_decodes_unicode_without_accepting_native_wire_utf8() {
    assert_eq!(
        decode_utf8_xtext("alice\\x{20}例", 100),
        Err(ParseError::Syntax)
    );
    assert_eq!(
        decode_utf8_xtext("alice@\\x{4F8B}.test", 100),
        Ok("alice@例.test".as_bytes().to_vec())
    );
    assert_eq!(
        decode_utf8_xtext("a\\x{2B}\\x{3D}\\x{5C}", 100),
        Ok(b"a+=\\".to_vec())
    );
}

/// Embedded Unicode validation rejects malformed, forbidden, and non-scalar points.
#[test]
fn utf8_xtext_rejects_malformed_or_noncanonical_embedded_points() {
    for invalid in [
        "",
        "é",
        "a\\x{1}",
        "a\\x{0A}",
        "a\\x{D800}",
        "a\\x{110000}",
        "a\\x{000080}",
        "a\\x{000041}",
        "a\\x{4F8B",
        "a\\x{4F8B}b\\",
        "a\\x{4F8B}=b",
    ] {
        assert_eq!(
            decode_utf8_xtext(invalid, 100),
            Err(ParseError::Syntax),
            "{invalid}"
        );
    }
    assert_eq!(
        decode_utf8_xtext("a\\x{4f8b}", 100),
        Ok("a例".as_bytes().to_vec())
    );
    assert_eq!(decode_utf8_xtext("valid", 4), Err(ParseError::Syntax));
}
