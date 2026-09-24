//! Типизированный разбор LIST, LIST-EXTENDED и устаревшей команды LSUB.

use super::Command;
use super::folder::{ListPattern, ListReturnOptions, ListSelection};
use super::tokenizer::Token;

/// Имя начальной selection-опции после проверки сетевого atom.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectionOption {
    Subscribed,
    SpecialUse,
    RecursiveMatch,
    Remote,
}

/// Узкий типизированный набор LIST RETURN, чтобы неизвестный atom не дошёл до reactor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReturnOption {
    SpecialUse,
    Subscribed,
    Children,
    Status,
}

/// Разбирает LIST и не переносит необработанные имена опций в reactor.
pub fn parse(mut tokens: impl Iterator<Item = Token>) -> Command {
    match tokens.next() {
        Some(Token::ListStart) => parse_extended(&mut tokens),
        Some(first) => {
            let restored = std::iter::once(first).chain(tokens);
            parse_basic(restored)
        }
        None => Command::Unsupported,
    }
}

fn parse_extended(tokens: &mut impl Iterator<Item = Token>) -> Command {
    let Some(selection) = parse_selection(tokens) else {
        return Command::Unsupported;
    };
    let Some((pattern, returns)) = parse_pattern(tokens) else {
        return Command::Unsupported;
    };
    match (selection.subscribed, selection.special_use) {
        (false, false) => Command::List { pattern, returns },
        (true, false) | (false, true) | (true, true) => Command::ListSelection {
            pattern,
            selection,
            returns,
        },
    }
}

/// Собирает selection options как множество: порядок и повторы не меняют смысл.
fn parse_selection(tokens: &mut impl Iterator<Item = Token>) -> Option<ListSelection> {
    let mut selection = ListSelection::default();
    let mut remote = false;
    loop {
        match tokens.next() {
            Some(Token::ListEnd) => break,
            Some(Token::Text(name)) => match selection_option(&name)? {
                SelectionOption::Subscribed => selection.subscribed = true,
                SelectionOption::SpecialUse => selection.special_use = true,
                SelectionOption::RecursiveMatch => selection.recursive_match = true,
                SelectionOption::Remote => remote = true,
            },
            _ => return None,
        }
    }
    match (
        selection.recursive_match,
        selection.subscribed,
        selection.special_use,
    ) {
        (true, false, false) => None,
        (_, _, _) => {
            // В сервере пока нет модели удалённых mailbox, поэтому REMOTE не
            // добавляет отдельные имена и не меняет локальную выборку.
            let _remote_is_ignored_without_remote_mailboxes = remote;
            Some(selection)
        }
    }
}

/// Сравнивает имя atom только на сетевой границе и возвращает типизированный enum.
fn selection_option(name: &str) -> Option<SelectionOption> {
    match name.to_ascii_uppercase().as_str() {
        "SUBSCRIBED" => Some(SelectionOption::Subscribed),
        "SPECIAL-USE" => Some(SelectionOption::SpecialUse),
        "RECURSIVEMATCH" => Some(SelectionOption::RecursiveMatch),
        "REMOTE" => Some(SelectionOption::Remote),
        _ => None,
    }
}

fn parse_basic(mut tokens: impl Iterator<Item = Token>) -> Command {
    match parse_pattern_tail(&mut tokens, true) {
        Some((pattern, returns)) => Command::List { pattern, returns },
        None => Command::Unsupported,
    }
}

/// Разбирает reference, pattern и поддерживаемые RETURN options.
fn parse_pattern(
    tokens: &mut impl Iterator<Item = Token>,
) -> Option<(ListPattern, ListReturnOptions)> {
    parse_pattern_tail(tokens, false)
}

fn parse_pattern_tail(
    tokens: &mut impl Iterator<Item = Token>,
    allow_root: bool,
) -> Option<(ListPattern, ListReturnOptions)> {
    let reference = next_text(tokens)?;
    let pattern = next_text(tokens)?;
    match tokens.next() {
        None => {
            let resolved = super::list_reference::resolve(&reference, &pattern)?;
            match (allow_root, pattern.is_empty(), reference.is_empty()) {
                (false, true, true) => Some((
                    ListPattern::Exact(String::new()),
                    ListReturnOptions::default(),
                )),
                _ => Some((resolved, ListReturnOptions::default())),
            }
        }
        Some(Token::Text(return_name)) if return_name.eq_ignore_ascii_case("RETURN") => {
            parse_return(tokens, reference, pattern)
        }
        _ => None,
    }
}

fn parse_return(
    tokens: &mut impl Iterator<Item = Token>,
    reference: String,
    pattern: String,
) -> Option<(ListPattern, ListReturnOptions)> {
    if !matches!(tokens.next(), Some(Token::ListStart)) {
        return None;
    }
    let mut options = ListReturnOptions::default();
    loop {
        match tokens.next() {
            Some(Token::ListEnd) => break,
            Some(Token::Text(name)) => match return_option(&name)? {
                ReturnOption::SpecialUse => options.special_use = true,
                ReturnOption::Subscribed => options.subscribed = true,
                ReturnOption::Children => options.children = true,
                ReturnOption::Status => {
                    options.status = super::status::parse_return_attributes(tokens)?;
                }
            },
            _ => return None,
        }
    }
    match tokens.next() {
        None => Some((
            super::list_reference::resolve(&reference, &pattern)?,
            options,
        )),
        _ => None,
    }
}

/// Сравнение текста протокола ограничено parser boundary и возвращает enum.
fn return_option(name: &str) -> Option<ReturnOption> {
    match name.to_ascii_uppercase().as_str() {
        "SPECIAL-USE" => Some(ReturnOption::SpecialUse),
        "SUBSCRIBED" => Some(ReturnOption::Subscribed),
        "CHILDREN" => Some(ReturnOption::Children),
        "STATUS" => Some(ReturnOption::Status),
        _ => None,
    }
}

/// Сохраняет двухстрочную грамматику и прежнюю семантику LSUB.
pub fn parse_lsub(mut tokens: impl Iterator<Item = Token>) -> Command {
    match parse_pattern_tail(&mut tokens, false) {
        Some((pattern, _)) => Command::Lsub { pattern },
        None => Command::Unsupported,
    }
}

fn next_text(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    match tokens.next()? {
        Token::Text(value) | Token::Quoted(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Повтор selection option не меняет семантику команды.
    #[test]
    fn recursive_subscribed_selection_is_typed_and_deduplicated() {
        assert_eq!(
            crate::imap::parse("a LIST (RECURSIVEMATCH SUBSCRIBED SUBSCRIBED) \"\" \"%\"\r\n")
                .command,
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
            crate::imap::parse("a LIST (RECURSIVEMATCH) \"\" \"%\"\r\n").command,
            Command::Unsupported
        );
    }

    /// Базовый empty pattern продолжает обозначать root delimiter discovery.
    #[test]
    fn empty_basic_pattern_requests_root() {
        assert_eq!(
            crate::imap::parse("a LIST \"\" \"\"\r\n").command,
            Command::List {
                pattern: ListPattern::Root,
                returns: ListReturnOptions::default(),
            }
        );
    }

    /// RETURN CHILDREN и SUBSCRIBED принимаются как отдельные typed options.
    #[test]
    fn list_return_options_are_typed_and_deduplicated() {
        assert_eq!(
            crate::imap::parse("a LIST \"\" \"*\" RETURN (CHILDREN SUBSCRIBED CHILDREN)\r\n")
                .command,
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

    /// Пока не реализованные return data items отклоняются, а не игнорируются.
    #[test]
    fn unsupported_list_return_option_is_rejected() {
        assert_eq!(
            crate::imap::parse("a LIST \"\" \"*\" RETURN (UNKNOWN)\r\n").command,
            Command::Unsupported
        );
    }

    /// STATUS items typed; повторы схлопываются, а rev1 RECENT не подменяет rev2 набор.
    #[test]
    fn list_status_return_items_are_typed_and_deduplicated() {
        assert_eq!(
            crate::imap::parse(
                "a LIST () \"\" \"*\" RETURN (STATUS (MESSAGES UNSEEN MESSAGES SIZE))\r\n"
            )
            .command,
            Command::List {
                pattern: ListPattern::All,
                returns: ListReturnOptions {
                    status: vec![
                        super::super::status::StatusAttribute::Messages,
                        super::super::status::StatusAttribute::Unseen,
                        super::super::status::StatusAttribute::Size,
                    ],
                    ..ListReturnOptions::default()
                },
            }
        );
        assert_eq!(
            crate::imap::parse("a LIST \"\" \"*\" RETURN (STATUS (RECENT))\r\n").command,
            Command::Unsupported
        );
    }
}
