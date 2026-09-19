//! Доменное представление IMAP INTERNALDATE без зависимости от системной locale.

use std::time::{SystemTime, UNIX_EPOCH};

/// Момент времени в секундах Unix, пригодный для долговечного бинарного журнала.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MailDate(i64);

impl MailDate {
    /// Фиксирует текущее время один раз на границе операции доставки.
    pub fn now() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default();
        Self(i64::try_from(seconds).unwrap_or(i64::MAX))
    }

    pub fn from_unix_seconds(seconds: i64) -> Self {
        Self(seconds)
    }

    pub fn unix_seconds(self) -> i64 {
        self.0
    }

    /// Разбирает обязательную IMAP date-time и сразу применяет числовой offset.
    pub fn parse(value: &str) -> Option<Self> {
        let mut fields = value.split_ascii_whitespace();
        let date = fields.next()?;
        let time = fields.next()?;
        let zone = fields.next()?;
        if fields.next().is_some() {
            return None;
        }
        let (day, month, year) = parse_date(date)?;
        let (hour, minute, second) = parse_time(time)?;
        let offset = parse_zone(zone)?;
        let days = days_from_civil(year, month.number(), day)?;
        let local = days
            .checked_mul(86_400)?
            .checked_add(i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second))?;
        Some(Self(local.checked_sub(offset)?))
    }

    /// Формирует канонический UTC INTERNALDATE, не зависящий от timezone процесса.
    pub fn encode(self) -> String {
        let days = self.0.div_euclid(86_400);
        let seconds = self.0.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        let hour = seconds / 3_600;
        let minute = seconds.rem_euclid(3_600) / 60;
        let second = seconds.rem_euclid(60);
        format!(
            "{day:02}-{}-{year:04} {hour:02}:{minute:02}:{second:02} +0000",
            month.atom()
        )
    }
}

#[derive(Clone, Copy)]
enum Month {
    Jan,
    Feb,
    Mar,
    Apr,
    May,
    Jun,
    Jul,
    Aug,
    Sep,
    Oct,
    Nov,
    Dec,
}

impl Month {
    fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_uppercase().as_str() {
            "JAN" => Some(Self::Jan),
            "FEB" => Some(Self::Feb),
            "MAR" => Some(Self::Mar),
            "APR" => Some(Self::Apr),
            "MAY" => Some(Self::May),
            "JUN" => Some(Self::Jun),
            "JUL" => Some(Self::Jul),
            "AUG" => Some(Self::Aug),
            "SEP" => Some(Self::Sep),
            "OCT" => Some(Self::Oct),
            "NOV" => Some(Self::Nov),
            "DEC" => Some(Self::Dec),
            _ => None,
        }
    }

    fn number(self) -> u32 {
        match self {
            Self::Jan => 1,
            Self::Feb => 2,
            Self::Mar => 3,
            Self::Apr => 4,
            Self::May => 5,
            Self::Jun => 6,
            Self::Jul => 7,
            Self::Aug => 8,
            Self::Sep => 9,
            Self::Oct => 10,
            Self::Nov => 11,
            Self::Dec => 12,
        }
    }

    fn from_number(value: u32) -> Self {
        match value {
            1 => Self::Jan,
            2 => Self::Feb,
            3 => Self::Mar,
            4 => Self::Apr,
            5 => Self::May,
            6 => Self::Jun,
            7 => Self::Jul,
            8 => Self::Aug,
            9 => Self::Sep,
            10 => Self::Oct,
            11 => Self::Nov,
            _ => Self::Dec,
        }
    }

    fn atom(self) -> &'static str {
        match self {
            Self::Jan => "Jan",
            Self::Feb => "Feb",
            Self::Mar => "Mar",
            Self::Apr => "Apr",
            Self::May => "May",
            Self::Jun => "Jun",
            Self::Jul => "Jul",
            Self::Aug => "Aug",
            Self::Sep => "Sep",
            Self::Oct => "Oct",
            Self::Nov => "Nov",
            Self::Dec => "Dec",
        }
    }
}

fn parse_date(value: &str) -> Option<(u32, Month, i32)> {
    let mut parts = value.split('-');
    let day = parts.next()?.parse().ok()?;
    let month = Month::parse(parts.next()?)?;
    let year = parts.next()?.parse().ok()?;
    match (parts.next(), valid_day(year, month.number(), day)) {
        (None, true) => Some((day, month, year)),
        _ => None,
    }
}

fn parse_time(value: &str) -> Option<(u32, u32, u32)> {
    let mut parts = value.split(':');
    let result = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    match (parts.next(), result) {
        (None, (hour, minute, second)) if hour < 24 && minute < 60 && second < 60 => Some(result),
        _ => None,
    }
}

fn parse_zone(value: &str) -> Option<i64> {
    let (sign, digits) = match (value.strip_prefix('+'), value.strip_prefix('-')) {
        (Some(digits), _) => (1_i64, digits),
        (None, Some(digits)) => (-1_i64, digits),
        (None, None) => return None,
    };
    let (hours, minutes) = digits.split_at_checked(2)?;
    let (hours, minutes): (i64, i64) = (hours.parse().ok()?, minutes.parse().ok()?);
    match (digits.len(), hours < 24, minutes < 60) {
        (4, true, true) => Some(sign * (hours * 3_600 + minutes * 60)),
        _ => None,
    }
}

fn valid_day(year: i32, month: u32, day: u32) -> bool {
    let leap = match (
        year.rem_euclid(4),
        year.rem_euclid(100),
        year.rem_euclid(400),
    ) {
        (0, non_century, _) if non_century != 0 => true,
        (0, 0, 0) => true,
        _ => false,
    };
    let maximum = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=maximum).contains(&day)
}

/// Gregorian date -> Unix epoch days, алгоритм Hinnant с проверенной арифметикой.
fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    let adjusted_year = i64::from(year) - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = i64::from(month)
        + match month > 2 {
            true => -3,
            false => 9,
        };
    let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era.checked_mul(146_097)?
        .checked_add(day_of_era)?
        .checked_sub(719_468)
}

fn civil_from_days(days: i64) -> (i32, Month, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime
        + match month_prime < 10 {
            true => 3,
            false => -9,
        };
    year += i64::from(month <= 2);
    (
        i32::try_from(year).unwrap_or_default(),
        Month::from_number(month as u32),
        day as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_offset_and_formats_utc() {
        let date = MailDate::parse("19-Sep-2026 12:34:56 +0300").expect("date");
        assert_eq!(date.encode(), "19-Sep-2026 09:34:56 +0000");
    }

    #[test]
    fn validates_leap_day() {
        assert!(MailDate::parse("29-Feb-2024 00:00:00 +0000").is_some());
        assert!(MailDate::parse("29-Feb-2023 00:00:00 +0000").is_none());
    }
}
