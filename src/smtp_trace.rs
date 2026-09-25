//! Формирование служебных SMTP-заголовков на границе локальной доставки.

use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

/// Добавляет `Return-Path` и `Received`, сохраняя исходные байты MIME без пересборки.
pub fn prepend(message: &[u8], reverse_path: &str, received_at: SystemTime) -> io::Result<Vec<u8>> {
    let seconds = received_at
        .duration_since(UNIX_EPOCH)
        .map_err(|_| io::Error::other("received time precedes Unix epoch"))?
        .as_secs();
    let path = match reverse_path.is_empty() {
        true => "<>".to_owned(),
        false => format!("<{reverse_path}>"),
    };
    let trace = format!(
        "Return-Path: {path}\r\nReceived: by litterkeep.test with ESMTP; {}\r\n",
        rfc5322_utc(seconds)
    );
    let mut result = Vec::with_capacity(trace.len().saturating_add(message.len()));
    result.extend_from_slice(trace.as_bytes());
    result.extend_from_slice(message);
    Ok(result)
}

/// Преобразует Unix time в фиксированный RFC 5322 UTC date-time за O(1).
pub(crate) fn rfc5322_utc(seconds: u64) -> String {
    let days = seconds / 86_400;
    let day_seconds = seconds % 86_400;
    let hour = day_seconds / 3_600;
    let minute = day_seconds % 3_600 / 60;
    let second = day_seconds % 60;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{:02} {} {year:04} {hour:02}:{minute:02}:{second:02} +0000",
        day,
        month_name(month)
    )
}

/// Алгоритм civil-from-days Говарда Хиннанта без таблиц и индексирования.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month as u32, day as u32)
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "Invalid",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepends_envelope_headers_without_rewriting_message() {
        let raw = b"From: Alice <alice@test>\r\nBcc: hidden@test\r\n\r\nbody\x00";
        let actual = prepend(raw, "alice@test", UNIX_EPOCH).expect("trace");
        let prefix = b"Return-Path: <alice@test>\r\nReceived: by litterkeep.test with ESMTP; 01 Jan 1970 00:00:00 +0000\r\n";
        assert!(actual.starts_with(prefix));
        assert!(actual.ends_with(raw));
    }

    #[test]
    fn null_reverse_path_is_preserved() {
        let actual = prepend(b"Subject: bounce\r\n\r\n", "", UNIX_EPOCH).expect("trace");
        assert!(actual.starts_with(b"Return-Path: <>\r\n"));
    }
}
