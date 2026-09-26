//! Проверка SMTP клиентского имени в командах EHLO и HELO.
use super::{ParseError, address, address_literal};

/// Проверяет EHLO: DNS Domain либо корректный SMTP address-literal.
pub(super) fn ehlo(argument: &str) -> Result<String, ParseError> {
    let host = one_argument(argument)?;
    let is_domain = address::valid_domain(host);
    let is_address_literal = matches!(
        address_literal::parse_domain(host),
        address_literal::LiteralForm::Valid
    );
    match (is_domain, is_address_literal) {
        (true, _) | (_, true) => Ok(host.to_owned()),
        (false, false) => Err(ParseError::Syntax),
    }
}

/// Проверяет HELO: RFC 5321 допускает Domain, но не address-literal.
pub(super) fn helo(argument: &str) -> Result<String, ParseError> {
    let host = one_argument(argument)?;
    match address::valid_domain(host) {
        true => Ok(host.to_owned()),
        false => Err(ParseError::Syntax),
    }
}

/// Требует единственный непустой аргумент, отделённый только одиночным SP.
fn one_argument(argument: &str) -> Result<&str, ParseError> {
    let mut parts = argument.split(' ');
    let Some(host) = parts.next() else {
        return Err(ParseError::Syntax);
    };
    if host.is_empty() {
        return Err(ParseError::Syntax);
    }
    if parts.next().is_some() {
        return Err(ParseError::Syntax);
    }
    Ok(host)
}

#[cfg(test)]
#[path = "greeting_tests.rs"]
mod tests;
