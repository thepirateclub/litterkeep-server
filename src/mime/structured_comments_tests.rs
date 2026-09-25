//! Проверки двух режимов обработки RFC 822 comments.

use super::{remove, remove_preserving_adjacency};

#[test]
fn normal_mode_separates_tokens_around_comments() {
    assert_eq!(remove("token(comment)value").as_deref(), Ok("token  value"));
}

#[test]
fn adjacency_mode_removes_comment_without_deleting_real_whitespace() {
    assert_eq!(
        remove_preserving_adjacency("1.(comment)0").as_deref(),
        Ok("1.0")
    );
    assert_eq!(remove_preserving_adjacency("1 . 0").as_deref(), Ok("1 . 0"));
}

#[test]
fn adjacency_mode_keeps_comments_inside_quoted_strings_as_data() {
    assert_eq!(
        remove_preserving_adjacency("\"a(comment)b\"").as_deref(),
        Ok("\"a(comment)b\"")
    );
}

#[test]
fn both_modes_reject_unterminated_nested_comments() {
    assert!(remove("value (outer(inner)").is_err());
    assert!(remove_preserving_adjacency("value (outer(inner)").is_err());
}
