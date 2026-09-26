//! Регрессионные проверки базовых LIST и LIST-EXTENDED форм.

use super::folder::{ListPattern, ListReturnOptions, ListSelection};
use super::{Command, parse};

/// Повтор selection option не меняет семантику команды.
#[test]
fn recursive_subscribed_selection_is_typed_and_deduplicated() {
    assert_eq!(
        parse("a LIST (RECURSIVEMATCH SUBSCRIBED SUBSCRIBED) \"\" \"%\"\r\n").command,
        Command::ListSelection {
            pattern: ListPattern::OneLevel,
            selection: ListSelection {
                subscribed: true,
                special_use: false,
                recursive_match: true,
            },
            returns: ListReturnOptions::default(),
        }
    );
}

/// RECURSIVEMATCH без фильтрующей selection option отклоняется.
#[test]
fn recursive_match_alone_is_rejected() {
    assert_eq!(
        parse("a LIST (RECURSIVEMATCH) \"\" \"%\"\r\n").command,
        Command::Unsupported
    );
}

/// Пустой базовый pattern продолжает обозначать root delimiter discovery.
#[test]
fn empty_basic_pattern_requests_root() {
    assert_eq!(
        parse("a LIST \"\" \"\"\r\n").command,
        Command::List {
            pattern: ListPattern::Root,
            returns: ListReturnOptions::default(),
        }
    );
}

/// RETURN CHILDREN и SUBSCRIBED являются отдельными typed options.
#[test]
fn list_return_options_are_typed_and_deduplicated() {
    assert_eq!(
        parse("a LIST \"\" \"*\" RETURN (CHILDREN SUBSCRIBED CHILDREN)\r\n").command,
        Command::List {
            pattern: ListPattern::All,
            returns: ListReturnOptions {
                subscribed: true,
                special_use: false,
                children: true,
                status: Vec::new(),
            },
        }
    );
}

/// Не реализованные return data items отвергаются, а не игнорируются.
#[test]
fn unsupported_list_return_option_is_rejected() {
    assert_eq!(
        parse("a LIST \"\" \"*\" RETURN (UNKNOWN)\r\n").command,
        Command::Unsupported
    );
}

/// STATUS return items типизированы, deduplicated, и rev1 RECENT отвергается.
#[test]
fn list_status_return_items_are_typed_and_deduplicated() {
    assert_eq!(
        parse(
            "a LIST () \"\" \"*\" RETURN (STATUS (MESSAGES UNSEEN MESSAGES SIZE APPENDLIMIT))\r\n"
        )
        .command,
        Command::List {
            pattern: ListPattern::All,
            returns: ListReturnOptions {
                status: vec![
                    super::status::StatusAttribute::Messages,
                    super::status::StatusAttribute::Unseen,
                    super::status::StatusAttribute::Size,
                    super::status::StatusAttribute::AppendLimit,
                ],
                ..ListReturnOptions::default()
            },
        }
    );
    assert_eq!(
        parse("a LIST \"\" \"*\" RETURN (STATUS (RECENT))\r\n").command,
        Command::Unsupported
    );
}
