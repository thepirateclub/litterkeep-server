//! Типизированный SEARCH RETURN и форматирование данных ESEARCH.

use super::tokenizer::Token;
use std::collections::VecDeque;

/// Запрошенные поля ESEARCH; пустой RETURN означает стандартный набор ALL.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReturnItem {
    All,
    Count,
    Min,
    Max,
    Save,
    Partial(super::partial_range::PartialRange),
}

/// Отличает совместимый legacy SEARCH от явно запрошенного ESEARCH.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResultRequest {
    Legacy,
    Extended(Vec<ReturnItem>),
}

impl ResultRequest {
    /// Возвращает набор опций по умолчанию для RETURN без аргументов.
    pub fn extended(items: Vec<ReturnItem>) -> Self {
        match items.is_empty() {
            true => Self::Extended(vec![ReturnItem::All]),
            false => Self::Extended(items),
        }
    }

    /// Сообщает, должен ли успешный SEARCH заменить session search result.
    pub fn saves_result(&self) -> bool {
        matches!(self, Self::Extended(items) if items.contains(&ReturnItem::Save))
    }
}

/// Разбирает необязательный RETURN clause, отвергая неизвестные параметры.
pub(super) fn parse_prefix(tokens: &mut VecDeque<Token>) -> Option<ResultRequest> {
    let requested = match tokens.front() {
        Some(Token::Text(value)) => value.eq_ignore_ascii_case("RETURN"),
        _ => false,
    };
    match requested {
        false => Some(ResultRequest::Legacy),
        true => {
            tokens.pop_front();
            parse_return_list(tokens).map(ResultRequest::extended)
        }
    }
}

fn parse_return_list(tokens: &mut VecDeque<Token>) -> Option<Vec<ReturnItem>> {
    match tokens.pop_front()? {
        Token::ListStart => parse_items(tokens),
        Token::ListEnd | Token::Text(_) | Token::Quoted(_) | Token::Literal(_) => None,
    }
}

fn parse_items(tokens: &mut VecDeque<Token>) -> Option<Vec<ReturnItem>> {
    let mut items = Vec::new();
    loop {
        match tokens.pop_front()? {
            Token::ListEnd => return Some(items),
            Token::Text(value) => {
                let item = match value.eq_ignore_ascii_case("PARTIAL") {
                    true => match tokens.pop_front()? {
                        Token::Text(range) => {
                            ReturnItem::Partial(super::partial_range::PartialRange::parse(&range)?)
                        }
                        _ => return None,
                    },
                    false => parse_item(&value)?,
                };
                if conflicts(&items, &item) {
                    return None;
                }
                match items.contains(&item) {
                    true => return None,
                    false => items.push(item),
                }
            }
            Token::Quoted(_) | Token::Literal(_) | Token::ListStart => return None,
        }
    }
}

fn conflicts(items: &[ReturnItem], candidate: &ReturnItem) -> bool {
    let partial_already_requested = items
        .iter()
        .any(|item| matches!(item, ReturnItem::Partial(_)));
    let all_already_requested = items.contains(&ReturnItem::All);
    matches!(
        (candidate, partial_already_requested, all_already_requested),
        (ReturnItem::Partial(_), true, _)
            | (ReturnItem::Partial(_), _, true)
            | (ReturnItem::All, true, _)
    )
}

fn parse_item(value: &str) -> Option<ReturnItem> {
    match value.to_ascii_uppercase().as_str() {
        "ALL" => Some(ReturnItem::All),
        "COUNT" => Some(ReturnItem::Count),
        "MIN" => Some(ReturnItem::Min),
        "MAX" => Some(ReturnItem::Max),
        "SAVE" => Some(ReturnItem::Save),
        _ => None,
    }
}

/// Сжимает возрастающую последовательность в sequence-set без индексирования.
pub fn sequence_set(values: &[u64]) -> String {
    let mut ranges = Vec::new();
    let mut iter = values.iter().copied();
    let Some(mut start) = iter.next() else {
        return String::new();
    };
    let mut end = start;
    for value in iter {
        match value == end.saturating_add(1) {
            true => end = value,
            false => {
                ranges.push(format_range(start, end));
                start = value;
                end = value;
            }
        }
    }
    ranges.push(format_range(start, end));
    ranges.join(",")
}

fn format_range(start: u64, end: u64) -> String {
    match start == end {
        true => start.to_string(),
        false => format!("{start}:{end}"),
    }
}

#[cfg(test)]
#[path = "search_result_tests.rs"]
mod tests;
