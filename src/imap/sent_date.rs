//! Ограниченный, строгий разбор календарной даты из RFC 5322 `Date`.

use super::date_zone::{DateTimeProfile, normalized_zone};
use super::search_date::SearchDate;
use crate::mail_date::MailDate;

/// Извлекает UTC-день RFC 5322 Date с numeric или допустимой obsolete timezone.
///
/// Поддерживаются необязательный day-of-week, календарная проверка,
/// секунды, известные зоны RFC 5322 §4.3 и unknown-offset fallback.
/// Синтаксически некорректная или неоднозначно широкая зона отвергается.
pub fn parse(value: &[u8]) -> Option<SearchDate> {
    parse_with_profile(value, DateTimeProfile::Rfc5322)
}

/// Разбирает RFC 2183 quoted-date-time, где допустима только числовая зона.
pub(crate) fn parse_numeric_timezone(value: &[u8]) -> Option<SearchDate> {
    parse_with_profile(value, DateTimeProfile::Rfc2183NumericZone)
}

/// Применяет синтаксис и политику зоны выбранного RFC-профиля.
fn parse_with_profile(value: &[u8], profile: DateTimeProfile) -> Option<SearchDate> {
    let value = std::str::from_utf8(value).ok()?;
    let value = crate::mime::structured_comments::remove(value).ok()?;
    let value = crate::mime::wsp::trim(&value);
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
    if !valid_day(day) {
        return None;
    }
    let year = normalized_year(fields.next()?)?;
    let time = normalized_time(fields.next()?)?;
    let zone = normalized_zone(fields.next()?, profile)?;
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

/// Проверяет RFC day: одна или две десятичные ASCII-цифры.
fn valid_day(value: &str) -> bool {
    match value.len() {
        1 | 2 => value.bytes().all(|octet| octet.is_ascii_digit()),
        _ => false,
    }
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
    match (
        valid_clock_field(hour),
        valid_clock_field(minute),
        fields.next(),
        fields.next(),
    ) {
        (true, true, None, None) => Some(format!("{hour}:{minute}:00")),
        (true, true, Some("60"), None) => Some(format!("{hour}:{minute}:59")),
        (true, true, Some(second), None) if valid_clock_field(second) => {
            Some(format!("{hour}:{minute}:{second}"))
        }
        _ => None,
    }
}

/// Проверяет фиксированную RFC ширину hour, minute или second.
fn valid_clock_field(value: &str) -> bool {
    match value.len() {
        2 => value.bytes().all(|octet| octet.is_ascii_digit()),
        _ => false,
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
    fn rejects_unclassifiable_zone_extra_fields_and_invalid_weekday() {
        assert!(parse(b"Mon, 2 Feb 2026 12:30:00 UNKNOWN").is_none());
        assert!(parse(b"Bad, 2 Feb 2026 12:30:00 +0000").is_none());
        assert!(parse(b"2 Feb 2026 12:30:00 +0000 trailing").is_none());
    }

    /// RFC 5322 требует, чтобы weekday совпадал с локальной календарной датой.
    #[test]
    fn rejects_weekday_that_disagrees_with_calendar_date() {
        assert!(parse(b"Thu, 21 Nov 1997 09:55:06 -0600").is_none());
        assert!(parse(b"Fri, 21 Nov 1997 09:55:06 -0600").is_some());
    }

    /// RFC 822 CFWS comments не меняют компоненты корректной даты.
    #[test]
    fn accepts_nested_comments_between_date_time_components() {
        let expected = SearchDate::parse("21-Nov-1997").expect("date");
        assert_eq!(
            parse(b"Fri (weekday (note)), (day) 21 (month) Nov (year) 97 (time) 09:55:06 (zone) -0600"),
            Some(expected)
        );
    }

    /// Незакрытый comment не превращается в частично распознанную дату.
    #[test]
    fn rejects_unterminated_comment_in_date_time() {
        assert!(parse(b"Fri, 21 Nov 1997 09:55:06 (missing zone -0600").is_none());
    }

    /// RFC 5322 §4.3 зоны США применяются с заданными фиксированными offsets.
    #[test]
    fn interprets_obsolete_north_american_zones() {
        let day = SearchDate::parse("2-Feb-2026").expect("February 2");
        for value in [
            b"Mon, 2 Feb 2026 00:30:00 UT".as_slice(),
            b"Mon, 2 Feb 2026 00:30:00 gmt".as_slice(),
            b"Mon, 2 Feb 2026 00:30:00 EST".as_slice(),
            b"Mon, 2 Feb 2026 00:30:00 EDT".as_slice(),
            b"Mon, 2 Feb 2026 00:30:00 CST".as_slice(),
            b"Mon, 2 Feb 2026 00:30:00 CDT".as_slice(),
            b"Mon, 2 Feb 2026 00:30:00 MST".as_slice(),
            b"Mon, 2 Feb 2026 00:30:00 MDT".as_slice(),
            b"Mon, 2 Feb 2026 00:30:00 PST".as_slice(),
            b"Mon, 2 Feb 2026 00:30:00 PDT".as_slice(),
        ] {
            assert_eq!(parse(value), Some(day), "zone in {value:?}");
        }
    }

    /// RFC 5322 рекомендует считать военные зоны неизвестным `-0000`.
    #[test]
    fn military_zones_use_unknown_offset_and_exclude_j() {
        let day = SearchDate::parse("1-Feb-2026").expect("February 1");
        assert_eq!(parse(b"Sun, 1 Feb 2026 23:30:00 A"), Some(day));
        assert_eq!(parse(b"Sun, 1 Feb 2026 23:30:00 Z"), Some(day));
        assert!(parse(b"Sun, 1 Feb 2026 23:30:00 J").is_none());
    }

    /// Проверяет весь RFC 5322 military-zone диапазон в обоих регистрах.
    #[test]
    fn accepts_every_defined_military_zone_case_insensitively() {
        let day = SearchDate::parse("1-Feb-2026").expect("February 1");
        for zone in b'A'..=b'Z' {
            if let b'J' = zone {
                continue;
            }
            for spelling in [zone, zone.to_ascii_lowercase()] {
                let value = format!("Sun, 1 Feb 2026 23:30:00 {}", char::from(spelling));
                assert_eq!(parse(value.as_bytes()), Some(day), "zone {spelling:?}");
            }
        }
    }

    /// Неизвестная историческая буквенная зона сравнивается как `-0000`.
    #[test]
    fn unknown_three_to_five_letter_zones_use_unknown_offset() {
        let day = SearchDate::parse("2-Feb-2026").expect("February 2");
        assert_eq!(parse(b"Mon, 2 Feb 2026 12:30:00 XYZ"), Some(day));
        assert_eq!(parse(b"Mon, 2 Feb 2026 12:30:00 abcde"), Some(day));
        assert!(parse(b"Mon, 2 Feb 2026 12:30:00 UNKNOWN").is_none());
    }

    /// RFC 2183 сохраняет numeric-only policy даже для общей RFC 5322 даты.
    #[test]
    fn rfc2183_date_parameters_reject_obsolete_named_zones() {
        assert!(parse_numeric_timezone(b"Fri, 21 Nov 1997 09:55:06 EST").is_none());
        assert!(parse_numeric_timezone(b"Fri, 21 Nov 1997 09:55:06 XYZ").is_none());
        assert!(parse_numeric_timezone(b"Fri, 21 Nov 1997 09:55:06 -0500").is_some());
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

    /// Современная дата не принимает однозначные или чрезмерно широкие поля.
    #[test]
    fn rejects_date_and_time_fields_outside_rfc_widths() {
        for value in [
            b"Fri, 001 Nov 1997 09:55:06 -0600".as_slice(),
            b"Fri, 21 Nov 1997 9:55:06 -0600".as_slice(),
            b"Fri, 21 Nov 1997 09:5:06 -0600".as_slice(),
            b"Fri, 21 Nov 1997 09:55:6 -0600".as_slice(),
            b"Fri, 21 Nov 1997 009:55:06 -0600".as_slice(),
        ] {
            assert!(parse(value).is_none(), "accepted date-time: {value:?}");
        }
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
