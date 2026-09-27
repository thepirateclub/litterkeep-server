//! Разбор obsolete source-route из RFC 5322 angle-addr.

use super::envelope_address_scan::{split_commas, split_optional};

/// Удаляет маршрут, сохраняя только конечный addr-spec.
pub(super) fn strip_source_route(value: &str) -> Result<&str, &'static str> {
    match starts_with_route_domain(value) {
        false => Ok(value),
        true => {
            let (route, destination) = split_optional(value, ':')?;
            let destination = destination.ok_or("obsolete source-route has no terminator")?;
            validate_source_route(route)?;
            match destination.trim().is_empty() {
                true => Err("obsolete source-route has no destination"),
                false => Ok(destination.trim()),
            }
        }
    }
}

/// Находит первый route-domain после допустимых начальных запятых и CFWS.
fn starts_with_route_domain(value: &str) -> bool {
    value.trim_start_matches([' ', '\t', ',']).starts_with('@')
}

/// Проверяет первый route-domain и допустимые пустые позиции после запятой.
fn validate_source_route(route: &str) -> Result<(), &'static str> {
    let route = route.trim_start_matches([' ', '\t', ',']).trim();
    let mut domains = split_commas(route)?.into_iter();
    let first = domains
        .next()
        .ok_or("obsolete source-route must start with a domain")?;
    validate_route_domain(first, true)?;
    for domain in domains {
        validate_route_domain(domain, false)?;
    }
    Ok(())
}

/// Проверяет один route элемент; пустой разрешён только в необязательном хвосте.
fn validate_route_domain(value: &str, required: bool) -> Result<(), &'static str> {
    let domain = value.trim();
    match (domain.is_empty(), required) {
        (true, true) => Err("obsolete source-route must start with a domain"),
        (true, false) => Ok(()),
        (false, _) => match domain.strip_prefix('@') {
            Some(value) if !value.trim().is_empty() => Ok(()),
            Some(_) => Err("obsolete source-route contains an empty domain"),
            None => Err("obsolete source-route element has no at-sign"),
        },
    }
}
