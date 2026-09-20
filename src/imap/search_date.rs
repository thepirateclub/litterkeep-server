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
