//! Выбор и восстановление Content-Type до построения MIME-дерева.

use super::super::header_block::{HeaderField, HeaderName};
use super::super::parameters::{self, StructuredValue};

/// Применяет RFC 2045 §5.2 fallback, отвергая повторные поля.
pub(super) fn parse(fields: &[HeaderField]) -> Result<StructuredValue, &'static str> {
    let mut values = fields
        .iter()
        .filter(|field| matches!(field.kind(), HeaderName::ContentType));
    match values.next() {
        None => parameters::parse("text/plain"),
        Some(field) => match values.next() {
            Some(_) => Err("duplicate Content-Type field"),
            None => parse_single(&field.value),
        },
    }
}

/// Сохраняет wire-заголовок, а при повреждённом значении использует plain text.
fn parse_single(value: &[u8]) -> Result<StructuredValue, &'static str> {
    match std::str::from_utf8(value) {
        Ok(value) => match parameters::parse(value) {
            Ok(parsed) => Ok(parsed),
            Err(_) => parameters::parse("text/plain"),
        },
        Err(_) => parameters::parse("text/plain"),
    }
}
