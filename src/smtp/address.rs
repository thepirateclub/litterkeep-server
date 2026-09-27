//! Грамматика SMTP mailbox и локальных частей SMTP пути.

/// Форма mailbox после синтаксической проверки SMTP адреса.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MailboxForm {
    NullReversePath,
    DotAtom,
    QuotedLocalPart,
    AddressLiteral,
    BarePostmaster,
    Invalid,
}

/// Типизированный результат проверки доменной части SMTP mailbox.
#[derive(Clone, Debug, Eq, PartialEq)]
enum DomainForm {
    DnsName(String),
    AddressLiteral,
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
/// Поэтому вызывающий слой получает unescaped local-part и канонический A-label домена.
pub(crate) fn canonical_parts(mailbox: &str) -> Option<(String, String)> {
    let separator = at_separator(mailbox)?;
    let (local_part, suffix) = mailbox.split_at(separator);
    let domain = suffix.strip_prefix('@')?;
    let canonical_local = canonical_local_part(local_part)?;
    match domain_form(domain) {
        DomainForm::DnsName(canonical_domain) => Some((canonical_local, canonical_domain)),
        DomainForm::AddressLiteral => Some((canonical_local, domain.to_owned())),
        DomainForm::Invalid => None,
    }
}

/// Определяет, требует ли полный mailbox параметр SMTPUTF8.
pub(crate) fn requires_smtp_utf8(mailbox: &str) -> bool {
    !mailbox.is_ascii()
}

/// Удаляет устаревший source route после строгой проверки его RFC-грамматики.
///
/// RFC 5321 требует принимать source routes ради совместимости, но игнорировать
/// их при доставке. Поэтому дальше по слоям передаётся только конечный mailbox.
pub(crate) fn without_source_route(path: &str) -> Option<&str> {
    match path.strip_prefix('@') {
        None => Some(path),
        Some(route_and_mailbox) => {
            let (route, mailbox) = route_and_mailbox.split_once(':')?;
            match valid_source_route(route) {
                true => Some(mailbox),
                false => None,
            }
        }
    }
}

/// Проверяет `@domain,@domain` без материализации списка переходов.
fn valid_source_route(route: &str) -> bool {
    let mut domains = route.split(',');
    match domains.next() {
        Some(first) if valid_domain(first) => domains.all(valid_routed_domain),
        _ => false,
    }
}

/// Проверяет каждый последующий `,@domain` как отдельный синтаксический элемент.
fn valid_routed_domain(route_item: &str) -> bool {
    match route_item.strip_prefix('@') {
        Some(domain) => valid_domain(domain),
        None => false,
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

/// Проверяет qtextSMTP с UTF8-non-ascii и ASCII quoted-pairSMTP.
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
            _ if !character.is_ascii() => decoded.push(character),
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
    let local_form = match local_part.strip_prefix('"') {
        Some(_) => canonical_local_part(local_part).map(|_| MailboxForm::QuotedLocalPart),
        None => valid_local_part(local_part).then_some(MailboxForm::DotAtom),
    }?;
    match domain_form(domain) {
        DomainForm::DnsName(_) => Some(local_form),
        DomainForm::AddressLiteral => Some(MailboxForm::AddressLiteral),
        DomainForm::Invalid => None,
    }
}

/// Разбирает имя или address-literal и возвращает typed protocol form.
fn domain_form(domain: &str) -> DomainForm {
    match crate::smtp::idna17::to_alabel_domain(domain) {
        Ok(canonical_domain) => DomainForm::DnsName(canonical_domain),
        Err(_) => match super::address_literal::parse_domain(domain) {
            super::address_literal::LiteralForm::Valid => DomainForm::AddressLiteral,
            super::address_literal::LiteralForm::Invalid => DomainForm::Invalid,
        },
    }
}

/// Проверяет atext и точки без пустых dot-atom компонентов.
fn valid_local_part(local_part: &str) -> bool {
    if local_part.is_empty() || local_part.len() > super::MAX_LOCAL_PART_SIZE {
        return false;
    }
    local_part.split('.').all(valid_atom)
}

/// Проверяет один атом на ASCII atext либо RFC 6531 UTF8-non-ascii.
fn valid_atom(atom: &str) -> bool {
    !atom.is_empty()
        && atom
            .chars()
            .all(|character| !character.is_ascii() || valid_atext(character as u8))
}

/// Классифицирует разрешённые ASCII символы без строковой ветвизации.
fn valid_atext(octet: u8) -> bool {
    octet.is_ascii_alphanumeric()
        || matches!(octet, b'!' | b'#'..=b'\'' | b'*'..=b'+' | b'-' | b'/' | b'=' | b'?' | b'^'..=b'`' | b'{'..=b'~')
}

/// Проверяет DNS labels и общий предел длины ASCII domain.
pub(super) fn valid_domain(domain: &str) -> bool {
    crate::smtp::idna17::is_valid_ascii_domain(domain)
}

#[cfg(test)]
#[path = "address_tests.rs"]
mod tests;
