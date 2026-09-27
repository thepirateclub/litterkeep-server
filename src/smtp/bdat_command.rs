//! Типизированный разбор размера и маркера SMTP BDAT по RFC 3030.

/// Описывает один чанк и признак последнего чанка сообщения.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BdatCommand {
    /// Точное число следующих за CRLF октетов тела.
    pub chunk_size: u64,
    /// `true` означает завершение сообщения после этого чанка.
    pub last: bool,
}

/// Закрытый набор BDAT marker-ов после лексического разбора.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BdatMarker {
    Intermediate,
    Last,
}

/// Разбирает единственную допустимую форму `BDAT n [LAST]`.
pub(super) fn parse(argument: &str) -> Result<BdatCommand, super::ParseError> {
    let (size, marker) = match argument.split_once(' ') {
        Some((size, marker)) => (size, marker),
        None => (argument, ""),
    };
    let chunk_size = parse_size(size)?;
    let last = matches!(parse_marker(marker)?, BdatMarker::Last);
    Ok(BdatCommand { chunk_size, last })
}

/// Принимает только непустые десятичные цифры без знака и посторонних байтов.
fn parse_size(value: &str) -> Result<u64, super::ParseError> {
    match !value.is_empty() && value.bytes().all(|octet| octet.is_ascii_digit()) {
        true => value.parse().map_err(|_| super::ParseError::Syntax),
        false => Err(super::ParseError::Syntax),
    }
}

/// Ограничивает необязательный аргумент единственным регистронезависимым LAST.
fn parse_marker(value: &str) -> Result<BdatMarker, super::ParseError> {
    match value.to_ascii_uppercase().as_str() {
        "" => Ok(BdatMarker::Intermediate),
        "LAST" => Ok(BdatMarker::Last),
        _ => Err(super::ParseError::Syntax),
    }
}

#[cfg(test)]
#[path = "bdat_command_tests.rs"]
mod tests;
