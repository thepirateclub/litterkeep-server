//! Разрешение LIST reference и pattern в каноническое имя mailbox.

use super::folder::ListPattern;

/// Классифицирует только формы wildcard, имеющие отдельное значение в LIST.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PatternKind {
    Empty,
    All,
    OneLevel,
    Other,
}

/// Объединяет относительную ссылку и шаблон по иерархии `/` сервера.
///
/// RFC 9051 оставляет смысл reference частично серверной реализации. Здесь
/// reference трактуется как контекст-папка: отсутствующий завершающий delimiter
/// добавляется перед pattern. Выходной pattern канонический и поэтому LIST
/// ответы содержат полные имена mailbox, а не относительные фрагменты.
#[cfg(test)]
fn resolve(reference: &str, pattern: &str) -> Option<ListPattern> {
    resolve_with_mode(reference, pattern, super::tokenizer::StringMode::Legacy)
}

/// Объединяет reference и pattern после декодирования выбранного wire режима.
pub(super) fn resolve_with_mode(
    reference: &str,
    pattern: &str,
    mode: super::tokenizer::StringMode,
) -> Option<ListPattern> {
    let reference = decode(reference, mode)?;
    let pattern = decode(pattern, mode)?;
    match (reference.is_empty(), pattern.is_empty()) {
        (true, true) => Some(ListPattern::Root),
        (false, true) => Some(ListPattern::RootReference(
            reference.trim_end_matches('/').to_owned(),
        )),
        (true, false) => Some(standalone(pattern)),
        (false, false) => Some(joined(reference, pattern)),
    }
}

/// Сохраняет UTF-8 scalar в согласованном режиме и legacy modified UTF-7 иначе.
fn decode(value: &str, mode: super::tokenizer::StringMode) -> Option<String> {
    match mode {
        super::tokenizer::StringMode::Legacy => super::modified_utf7::decode(value).ok(),
        super::tokenizer::StringMode::Utf8Accept => {
            super::mailbox_utf8::validate(value).ok()?;
            Some(value.to_owned())
        }
    }
}

/// Сохраняет специальные варианты корня и одиночных wildcard.
fn standalone(pattern: String) -> ListPattern {
    match classify(&pattern) {
        PatternKind::All => ListPattern::All,
        PatternKind::OneLevel => ListPattern::OneLevel,
        PatternKind::Empty | PatternKind::Other => wildcard_or_exact(pattern),
    }
}

/// Сопоставляет последовательность wildcard-символов без строковых условий.
fn classify(pattern: &str) -> PatternKind {
    let mut characters = pattern.chars();
    match (characters.next(), characters.next()) {
        (None, None) => PatternKind::Empty,
        (Some('*'), None) => PatternKind::All,
        (Some('%'), None) => PatternKind::OneLevel,
        _ => PatternKind::Other,
    }
}

/// Создаёт каноническое имя, применив context только к относительному шаблону.
fn joined(reference: String, pattern: String) -> ListPattern {
    let separator = match reference.ends_with('/') {
        true => "",
        false => "/",
    };
    let canonical = format!("{reference}{separator}{pattern}");
    wildcard_or_exact(canonical)
}

/// Отличает literal от wildcard pattern после UTF-7 декодирования.
fn wildcard_or_exact(pattern: String) -> ListPattern {
    match (pattern.contains('*'), pattern.contains('%')) {
        (false, false) => ListPattern::Exact(pattern),
        _ => ListPattern::Wildcard(pattern),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference с delimiter ограничивает `%` дочерним уровнем каталога.
    #[test]
    fn reference_prefix_is_canonical_and_keeps_wildcards() {
        assert_eq!(
            resolve("Invoices/", "%"),
            Some(ListPattern::Wildcard("Invoices/%".to_owned()))
        );
        assert_eq!(
            resolve("Invoices", "2026"),
            Some(ListPattern::Exact("Invoices/2026".to_owned()))
        );
    }

    /// Пустая reference сохраняет базовые wildcard и discovery корня.
    #[test]
    fn empty_reference_preserves_basic_patterns() {
        assert_eq!(resolve("", ""), Some(ListPattern::Root));
        assert_eq!(resolve("", "*"), Some(ListPattern::All));
        assert_eq!(resolve("", "%"), Some(ListPattern::OneLevel));
    }
}
