//! Проверка входящей ASCII-метки ACE по RFC 5891.

use super::UlabelError;

/// Декодирует и полностью проверяет предполагаемую IDNA A-label.
pub(super) fn decode(label: &str) -> Result<String, UlabelError> {
    match (label.len() <= 63, label.get(..4)) {
        (false, _) => return Err(UlabelError::EncodedLabelTooLong),
        (true, Some(prefix)) if prefix.eq_ignore_ascii_case("xn--") => {}
        _ => return Err(UlabelError::InvalidALabel),
    }
    let canonical_input = label.to_ascii_lowercase();
    let body = canonical_input.get(4..).ok_or(UlabelError::InvalidALabel)?;
    if body.is_empty() {
        return Err(UlabelError::InvalidALabel);
    }
    let ulabel = super::super::punycode::decode(body).map_err(|_| UlabelError::InvalidALabel)?;
    if ulabel.is_ascii() {
        return Err(UlabelError::InvalidALabel);
    }
    let canonical = super::ulabel::to_alabel(&ulabel).map_err(|_| UlabelError::InvalidALabel)?;
    match canonical == canonical_input {
        true => Ok(ulabel),
        false => Err(UlabelError::InvalidALabel),
    }
}

#[cfg(test)]
#[path = "alabel_tests.rs"]
mod tests;
