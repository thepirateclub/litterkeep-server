//! Ограниченный, строгий разбор календарной даты из RFC 5322 `Date`.

use super::search_date::SearchDate;
use crate::mail_date::MailDate;

/// Извлекает UTC-день отправки из корректного базового RFC 5322 date-time.
///
/// Поддерживаются необязательный day-of-week, обязательная числовая timezone и
/// время с секундами либо без них. Obsolete named zones намеренно отвергаются,
/// чтобы сомнительная дата не превращалась в ложное SEARCH-совпадение.
pub fn parse(value: &[u8]) -> Option<SearchDate> {
    let value = crate::mime::wsp::trim(std::str::from_utf8(value).ok()?);
    let (weekday, date_time) = match value.split_once(',') {
        Some((weekday, tail)) => (
            Some(crate::mime::wsp::trim(weekday)),
            crate::mime::wsp::trim(tail),
        ),
        None => (None, value),
    };
    let mut fields = date_time.split_ascii_whitespace();
    let day = fields.next()?;
    let month = fields.next()?;
    let year = normalized_year(fields.next()?)?;
    let time = normalized_time(fields.next()?)?;
    let zone = fields.next()?;
    if fields.next().is_some() {
        return None;
    }
    if let Some(value) = weekday {
        let date = SearchDate::parse(&format!("{day}-{month}-{year}"))?;
        if !date.weekday_matches(value) {
            return None;
        }
    }
    let imap_date = format!("{day}-{month}-{year} {time} {zone}");
    MailDate::parse(&imap_date).map(SearchDate::from_mail_date)
}

/// Нормализует obsolete 2/3-digit year по RFC 5322 §4.3.
fn normalized_year(value: &str) -> Option<String> {
    if !value.bytes().all(|octet| octet.is_ascii_digit()) {
        return None;
    }
    match value.len() {
        2 => match value.parse::<u32>().ok()? {
            year @ 0..=49 => Some((year + 2_000).to_string()),
            year @ 50..=99 => Some((year + 1_900).to_string()),
            _ => None,
        },
        3 => Some((value.parse::<u32>().ok()? + 1_900).to_string()),
        width if width >= 4 => match value.parse::<i32>().ok()? {
            year if year >= 1_900 => Some(value.to_owned()),
            _ => None,
        },
        _ => None,
    }
}

fn normalized_time(value: &str) -> Option<String> {
    let mut fields = value.split(':');
    let hour = fields.next()?;
    let minute = fields.next()?;
    match (fields.next(), fields.next()) {
        (None, None) => Some(format!("{hour}:{minute}:00")),
        (Some("60"), None) => Some(format!("{hour}:{minute}:59")),
        (Some(second), None) => Some(format!("{hour}:{minute}:{second}")),
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

    /// RFC 5322 требует, чтобы weekday совпадал с локальной календарной датой.
    #[test]
    fn rejects_weekday_that_disagrees_with_calendar_date() {
        assert!(parse(b"Thu, 21 Nov 1997 09:55:06 -0600").is_none());
        assert!(parse(b"Fri, 21 Nov 1997 09:55:06 -0600").is_some());
    }

    /// Допустимая секунда 60 сохраняет календарный день при date-only поиске.
    #[test]
    fn accepts_rfc5322_leap_second() {
        let date = SearchDate::parse("31-Dec-2016").expect("date");
        assert_eq!(parse(b"Sat, 31 Dec 2016 23:59:60 +0000"), Some(date));
    }

    /// RFC 5322 §4.3 задаёт pivot для obsolete двух- и трёхзначного года.
    #[test]
    fn interprets_obsolete_numeric_years_with_rfc_pivot() {
        let year_1997 = SearchDate::parse("21-Nov-1997").expect("1997");
        let year_2049 = SearchDate::parse("21-Nov-2049").expect("2049");
        let year_1950 = SearchDate::parse("21-Nov-1950").expect("1950");
        let year_2023 = SearchDate::parse("21-Nov-2023").expect("2023");

        assert_eq!(parse(b"Fri, 21 Nov 97 09:55:06 -0600"), Some(year_1997));
        assert_eq!(parse(b"21 Nov 49 09:55:06 +0000"), Some(year_2049));
        assert_eq!(parse(b"21 Nov 50 09:55:06 +0000"), Some(year_1950));
        assert_eq!(parse(b"21 Nov 123 09:55:06 +0000"), Some(year_2023));
        assert!(parse(b"21 Nov 9 09:55:06 +0000").is_none());
    }

    #[test]
    fn rejects_unicode_whitespace_where_rfc5322_cfws_is_required() {
        for value in [
            "\u{00a0}2 Feb 2026 12:30:00 +0000",
            "Mon\u{00a0}, 2 Feb 2026 12:30:00 +0000",
            "Mon,\u{00a0}2 Feb 2026 12:30:00 +0000",
            "2 Feb 2026 12:30:00 +0000\u{00a0}",
        ] {
            assert!(
                parse(value.as_bytes()).is_none(),
                "accepted date: {value:?}"
            );
        }
    }
}
