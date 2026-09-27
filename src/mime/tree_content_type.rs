//! Выбор и восстановление Content-Type до построения MIME-дерева.

use super::super::header_block::{HeaderField, HeaderName};
use super::super::parameters::{self, MediaType, StructuredValue};

/// Контекстный MIME default для entity без поля Content-Type.
#[derive(Clone, Copy)]
pub(super) enum DefaultMediaType {
    PlainText,
    MessageRfc822,
}

/// Применяет RFC 2045 §5.2 fallback к отсутствующему или неверному полю.
pub(super) fn parse(
    fields: &[HeaderField],
    default: DefaultMediaType,
) -> Result<StructuredValue, &'static str> {
    let mut values = fields
        .iter()
        .filter(|field| matches!(field.kind(), HeaderName::ContentType));
    match values.next() {
        None => Ok(default_value(default)),
        Some(field) => match values.next() {
            Some(_) => Ok(default_value(DefaultMediaType::PlainText)),
            None => parse_single(&field.value),
        },
    }
}

/// Создаёт default value без повторного разбора протокольной строки.
fn default_value(default: DefaultMediaType) -> StructuredValue {
    let media_type = match default {
        DefaultMediaType::PlainText => MediaType::TextPlain,
        DefaultMediaType::MessageRfc822 => MediaType::MessageRfc822,
    };
    StructuredValue {
        media_type,
        parameters: Default::default(),
        parameter_languages: Default::default(),
    }
}

/// Сохраняет wire-заголовок, а при повреждённом значении использует plain text.
fn parse_single(value: &[u8]) -> Result<StructuredValue, &'static str> {
    match std::str::from_utf8(value) {
        Ok(value) => match parameters::parse(value) {
            Ok(parsed) => Ok(parsed),
            Err(_) => Ok(default_value(DefaultMediaType::PlainText)),
        },
        Err(_) => Ok(default_value(DefaultMediaType::PlainText)),
    }
}
