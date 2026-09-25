//! Типизированный разбор диапазона языка для POP3 RFC 6856 LANG.

/// Запрос языка, разделяющий выбор языка и валидный неподдержанный диапазон.
#[derive(Debug, Eq, PartialEq)]
pub enum LanguageRequest {
    Listing,
    Preferred,
    English,
    Unsupported,
}

/// Разбирает один RFC 4647 basic language range без строкового FSM.
pub fn parse(argument: Option<&str>, extra: Option<&str>) -> Option<LanguageRequest> {
    match (argument, extra) {
        (None, None) => Some(LanguageRequest::Listing),
        (Some("*"), None) => Some(LanguageRequest::Preferred),
        (Some(value), None) if valid_basic_range(value) => Some(language(value)),
        _ => None,
    }
}

/// Проверяет ASCII форму basic range: alpha subtag, затем alphanumeric subtags.
fn valid_basic_range(value: &str) -> bool {
    let mut subtags = value.split('-');
    let Some(primary) = subtags.next() else {
        return false;
    };
    (1..=8).contains(&primary.len())
        && primary.bytes().all(|byte| byte.is_ascii_alphabetic())
        && subtags.all(|subtag| {
            (1..=8).contains(&subtag.len())
                && subtag.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

/// Сопоставляет range с единственной локалью, объявленной сервером.
fn language(value: &str) -> LanguageRequest {
    match value.eq_ignore_ascii_case("en") {
        true => LanguageRequest::English,
        false => LanguageRequest::Unsupported,
    }
}
