//! Ограниченный, строгий разбор календарной даты из RFC 5322 `Date`.

use super::search_date::SearchDate;
use crate::mail_date::MailDate;

/// Извлекает UTC-день отправки из корректного базового RFC 5322 date-time.
///
/// Поддерживаются необязательный day-of-week, обязательная числовая timezone и
/// время с секундами либо без них. Obsolete named zones намеренно отвергаются,
/// чтобы сомнительная дата не превращалась в ложное SEARCH-совпадение.
pub fn parse(value: &[u8]) -> Option<SearchDate> {
    let value = std::str::from_utf8(value).ok()?.trim();
    let date_time = match value.split_once(',') {
        Some((weekday, tail)) => match valid_weekday(weekday.trim()) {
            true => tail.trim(),
            false => return None,
        },
        None => value,
    };
    let mut fields = date_time.split_ascii_whitespace();
    let day = fields.next()?;
    let month = fields.next()?;
    let year = fields.next()?;
    let time = normalized_time(fields.next()?)?;
    let zone = fields.next()?;
    if fields.next().is_some() {
        return None;
    }
    let imap_date = format!("{day}-{month}-{year} {time} {zone}");
    MailDate::parse(&imap_date).map(SearchDate::from_mail_date)
}

fn valid_weekday(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().as_str(),
        "MON" | "TUE" | "WED" | "THU" | "FRI" | "SAT" | "SUN"
    )
}

fn normalized_time(value: &str) -> Option<String> {
    match value.split(':').count() {
        2 => Some(format!("{value}:00")),
        3 => Some(value.to_owned()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_weekday_optional_seconds_and_timezone_day_shift() {
        let previous = SearchDate::parse("1-Feb-2026").expect("previous");
        let current = SearchDate::parse("2-Feb-2026").expect("current");
        assert_eq!(parse(b"Mon, 2 Feb 2026 00:30:00 +0100"), Some(previous));
        assert_eq!(parse(b"2 Feb 2026 12:30 +0000"), Some(current));
    }

    #[test]
    fn rejects_named_zone_extra_fields_and_invalid_weekday() {
        assert!(parse(b"Mon, 2 Feb 2026 12:30:00 GMT").is_none());
        assert!(parse(b"Bad, 2 Feb 2026 12:30:00 +0000").is_none());
        assert!(parse(b"2 Feb 2026 12:30:00 +0000 trailing").is_none());
    }
}
