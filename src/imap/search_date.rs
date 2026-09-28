//! Типизированная календарная дата IMAP SEARCH без времени и timezone.

use crate::mail_date::MailDate;

/// Номер UTC-дня, с которым сравнивается сохранённый INTERNALDATE.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SearchDate(i64);

impl SearchDate {
    /// Разбирает `date` из SEARCH в формате `day-month-year`.
    pub fn parse(value: &str) -> Option<Self> {
        let midnight = format!("{value} 00:00:00 +0000");
        MailDate::parse(&midnight).map(Self::from_mail_date)
    }

    /// Нормализует INTERNALDATE до календарного UTC-дня.
    pub fn from_mail_date(value: MailDate) -> Self {
        Self(value.unix_seconds().div_euclid(86_400))
    }

    /// Сверяет именованный день недели с локальной календарной датой.
    pub(crate) fn weekday_matches(self, candidate: &str) -> bool {
        match (Weekday::parse(candidate), Weekday::from_epoch_day(self.0)) {
            (Some(candidate), actual) => candidate == actual,
            (None, _) => false,
        }
    }
}

/// Семь дней недели для проверки семантики RFC 5322 date-time.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    /// Разбирает ровно один из RFC weekday token-ов без строковых fallback-ов.
    fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_uppercase().as_str() {
            "MON" => Some(Self::Monday),
            "TUE" => Some(Self::Tuesday),
            "WED" => Some(Self::Wednesday),
            "THU" => Some(Self::Thursday),
            "FRI" => Some(Self::Friday),
            "SAT" => Some(Self::Saturday),
            "SUN" => Some(Self::Sunday),
            _ => None,
        }
    }

    /// Вычисляет weekday за O(1) из Unix epoch day без таблиц или выделений.
    fn from_epoch_day(day: i64) -> Self {
        match day.rem_euclid(7) {
            0 => Self::Thursday,
            1 => Self::Friday,
            2 => Self::Saturday,
            3 => Self::Sunday,
            4 => Self::Monday,
            5 => Self::Tuesday,
            _ => Self::Wednesday,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_day_and_orders_valid_dates() {
        let first = SearchDate::parse("1-Feb-2026").expect("first");
        let second = SearchDate::parse("2-Feb-2026").expect("second");
        assert!(first < second);
        assert!(SearchDate::parse("30-Feb-2026").is_none());
    }
}
