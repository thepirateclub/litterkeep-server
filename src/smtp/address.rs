//! Грамматика ASCII SMTP mailbox для dot-atom и quoted local-part.

/// Форма mailbox после синтаксической проверки SMTP адреса.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MailboxForm {
    NullReversePath,
    DotAtom,
    QuotedLocalPart,
    BarePostmaster,
    Invalid,
}

/// Классифицирует mailbox без попытки определить существование аккаунта.
pub(super) fn classify_mailbox(mailbox: &str) -> MailboxForm {
    match mailbox {
        "" => MailboxForm::NullReversePath,
        _ if mailbox.eq_ignore_ascii_case("postmaster") => MailboxForm::BarePostmaster,
        _ => mailbox_form(mailbox).unwrap_or(MailboxForm::Invalid),
    }
}

/// Разделяет mailbox на эквивалентный local-part и проверенный домен.
///
/// RFC 5321 требует считать quoting эквивалентным при сопоставлении адреса.
/// Поэтому вызывающий слой получает unescaped local-part, но не изменённый домен.
pub(crate) fn canonical_parts(mailbox: &str) -> Option<(String, &str)> {
    let separator = at_separator(mailbox)?;
    let (local_part, suffix) = mailbox.split_at(separator);
    let domain = suffix.strip_prefix('@')?;
    let canonical_local = canonical_local_part(local_part)?;
    match valid_domain(domain) {
        true => Some((canonical_local, domain)),
        false => None,
    }
}

/// Находит единственный `@` вне quoted-string с однопроходным автоматом.
fn at_separator(mailbox: &str) -> Option<usize> {
    let mut quoted = false;
    let mut escaped = false;
    let mut separator = None;
    for (offset, character) in mailbox.char_indices() {
        match (quoted, escaped, character) {
            (true, true, _) => escaped = false,
            (true, false, '\\') => escaped = true,
            (true, false, '"') => quoted = false,
            (false, _, '"') if separator.is_none() => quoted = true,
            (false, _, '@') if separator.is_none() => separator = Some(offset),
            (false, _, '@') => return None,
            _ => {}
        }
    }
    match (quoted, escaped, separator) {
        (false, false, Some(offset)) => Some(offset),
        _ => None,
    }
}

/// Снимает quoted-string и quoted-pair после проверки RFC 5321 §4.1.2.
pub(crate) fn canonical_local_part(local_part: &str) -> Option<String> {
    if local_part.len() > super::MAX_LOCAL_PART_SIZE {
        return None;
    }
    match local_part
        .strip_prefix('"')
        .and_then(|part| part.strip_suffix('"'))
    {
        Some(quoted) => decode_quoted_local_part(quoted),
        None if valid_local_part(local_part) => Some(local_part.to_owned()),
        None => None,
    }
}

/// Проверяет qtextSMTP и quoted-pairSMTP и создаёт каноническую строку.
fn decode_quoted_local_part(quoted: &str) -> Option<String> {
    let mut characters = quoted.chars();
    let mut decoded = String::with_capacity(quoted.len());
    while let Some(character) = characters.next() {
        match character {
            '\\' => match characters.next() {
                Some(escaped) if escaped.is_ascii() && (32..=126).contains(&(escaped as u8)) => {
                    decoded.push(escaped)
                }
                _ => return None,
            },
            ' '..='!' | '#'..='[' | ']'..='~' => decoded.push(character),
            _ => return None,
        }
    }
    Some(decoded)
}

/// Валидирует mailbox и сохраняет типизированную форму local-part.
fn mailbox_form(mailbox: &str) -> Option<MailboxForm> {
    let separator = at_separator(mailbox)?;
    let (local_part, suffix) = mailbox.split_at(separator);
    let domain = suffix.strip_prefix('@')?;
    if !valid_domain(domain) {
        return None;
    }
    match local_part.strip_prefix('"') {
        Some(_) => canonical_local_part(local_part).map(|_| MailboxForm::QuotedLocalPart),
        None => valid_local_part(local_part).then_some(MailboxForm::DotAtom),
    }
}

/// Проверяет atext и точки без пустых dot-atom компонентов.
fn valid_local_part(local_part: &str) -> bool {
    if local_part.is_empty() || local_part.len() > super::MAX_LOCAL_PART_SIZE {
        return false;
    }
    local_part.split('.').all(valid_atom)
}

/// Проверяет один атом на допустимые RFC atext ASCII октеты.
fn valid_atom(atom: &str) -> bool {
    !atom.is_empty() && atom.bytes().all(valid_atext)
}

/// Классифицирует разрешённые ASCII символы без строковой ветвизации.
fn valid_atext(octet: u8) -> bool {
    octet.is_ascii_alphanumeric()
        || matches!(octet, b'!' | b'#'..=b'\'' | b'*'..=b'+' | b'-' | b'/' | b'=' | b'?' | b'^'..=b'`' | b'{'..=b'~')
}

/// Проверяет DNS labels и общий предел длины ASCII domain.
fn valid_domain(domain: &str) -> bool {
    !domain.is_empty()
        && domain.len() <= 255
        && domain.is_ascii()
        && domain.split('.').all(valid_domain_label)
}

/// Проверяет одну DNS-метку без Unicode или завершающей пустой метки.
fn valid_domain_label(label: &str) -> bool {
    let bytes = label.as_bytes();
    match bytes {
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
    }
}

#[cfg(test)]
#[path = "address_tests.rs"]
mod tests;
