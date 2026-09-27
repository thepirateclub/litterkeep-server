//! Преобразование доменного имени IDNA с учетом контекста всех меток.

use super::UlabelError;

const MAX_DOMAIN_OCTETS: usize = 253;

/// Преобразует Unicode/ASCII domain в каноническую последовательность A-label.
///
/// ASCII ACE-метки декодируются и проверяются повторным кодированием. Если хотя
/// бы одна метка содержит R, AL или AN, Bidi Rule применяется ко всем меткам.
pub fn to_alabel_domain(domain: &str) -> Result<String, UlabelError> {
    match (domain.is_empty(), domain.len() > 1024) {
        (true, _) => return Err(UlabelError::Empty),
        (false, true) => return Err(UlabelError::InputTooLong),
        (false, false) => {}
    }
    let mut ascii_labels = Vec::new();
    let mut unicode_labels = Vec::new();
    for label in domain.split('.') {
        let (ascii, unicode) = convert_label(label)?;
        ascii_labels.push(ascii);
        unicode_labels.push(unicode.chars().collect::<Vec<_>>());
    }
    validate_domain_bidi(&unicode_labels)?;
    let result = ascii_labels.join(".");
    match (ascii_labels.is_empty(), result.len() <= MAX_DOMAIN_OCTETS) {
        (true, _) => Err(UlabelError::Empty),
        (false, true) => Ok(result),
        (false, false) => Err(UlabelError::DomainTooLong),
    }
}

/// Проверяет SMTP-домен в ASCII wire form, включая каждую входящую A-label.
pub fn is_valid_ascii_domain(domain: &str) -> bool {
    domain.is_ascii() && to_alabel_domain(domain).is_ok()
}

/// Преобразует одну U-label, A-label или обычную ASCII LDH-метку.
fn convert_label(label: &str) -> Result<(String, String), UlabelError> {
    match (label.is_empty(), label.is_ascii()) {
        (true, _) => Err(UlabelError::Empty),
        (false, false) => {
            let ascii = super::ulabel::to_alabel(label)?;
            Ok((ascii, label.to_owned()))
        }
        (false, true) if has_ace_prefix(label) => {
            let unicode = super::alabel::decode(label)?;
            Ok((label.to_ascii_lowercase(), unicode))
        }
        (false, true) => {
            validate_ascii_label(label)?;
            Ok((label.to_ascii_lowercase(), label.to_owned()))
        }
    }
}

/// Применяет Bidi Rule ко всем меткам только для Bidi domain.
fn validate_domain_bidi(labels: &[Vec<char>]) -> Result<(), UlabelError> {
    let contains_rtl_label = labels.iter().any(|label| super::bidi::is_rtl_label(label));
    match contains_rtl_label {
        false => Ok(()),
        true => match labels
            .iter()
            .all(|label| super::bidi::validate_in_bidi_domain(label))
        {
            true => Ok(()),
            false => Err(UlabelError::BidiRule),
        },
    }
}

/// Проверяет длину и LDH-грамматику одной обычной ASCII DNS-метки.
fn validate_ascii_label(label: &str) -> Result<(), UlabelError> {
    let bytes = label.as_bytes();
    let valid = match bytes {
        [only] => only.is_ascii_alphanumeric(),
        [first, middle @ .., last] => {
            bytes.len() <= 63
                && first.is_ascii_alphanumeric()
                && last.is_ascii_alphanumeric()
                && middle
                    .iter()
                    .all(|octet| octet.is_ascii_alphanumeric() || *octet == b'-')
        }
        [] => false,
    };
    match valid {
        true => Ok(()),
        false => Err(UlabelError::InvalidAsciiLabel),
    }
}

/// Распознает ACE prefix без учета ASCII-регистра.
fn has_ace_prefix(label: &str) -> bool {
    label
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("xn--"))
}

#[cfg(test)]
#[path = "domain_tests.rs"]
mod tests;
