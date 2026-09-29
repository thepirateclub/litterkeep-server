//! Ограниченное хранилище и однопроходное раскрытие Sieve Variables.

use std::collections::HashMap;

/// RFC 5229 требует поддержку как минимум 128 различных имён.
pub const MAX_VARIABLES: usize = 128;
/// Минимальная длина имени, которую обязан поддержать сервер.
pub const MAX_VARIABLE_NAME_CHARS: usize = 32;
/// RFC 5229 задаёт минимум 4000 символов на значение переменной.
pub const MAX_VARIABLE_VALUE_CHARS: usize = 4_000;
/// Локальный предел раскрытой строки защищает от раздувания повторными ссылками.
pub const MAX_EXPANDED_STRING_CHARS: usize = 65_536;

/// Причина отказа при записи в ограниченное хранилище Variables.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VariableError {
    InvalidName,
    TooManyVariables,
    ExpansionLimit,
    UnsupportedCaptureIndex,
    UnsupportedNamespace,
}

/// Per-script именованные значения с ASCII-регистронезависимыми именами.
#[derive(Default)]
pub struct VariableStore {
    values: HashMap<String, String>,
}

impl VariableStore {
    /// Сохраняет значение, обрезая его по символам до RFC-минимума 4000.
    pub fn set(&mut self, name: &str, value: &str) -> Result<(), VariableError> {
        if !valid_name(name) {
            return Err(VariableError::InvalidName);
        }
        let key = name.to_ascii_lowercase();
        if !self.values.contains_key(&key) && self.values.len() >= MAX_VARIABLES {
            return Err(VariableError::TooManyVariables);
        }
        let bounded = value.chars().take(MAX_VARIABLE_VALUE_CHARS).collect();
        self.values.insert(key, bounded);
        Ok(())
    }

    /// Возвращает значение без различия регистра ASCII в имени.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    /// Раскрывает ссылки ровно одним проходом; неизвестные имена дают пустую строку.
    pub fn expand(&self, source: &str, captures: &[String]) -> Result<String, VariableError> {
        expand(source, &self.values, captures)
    }
}

/// Проверяет identifier из RFC 5228: ASCII-буква или `_`, затем буквы, цифры, `_`.
fn valid_name(name: &str) -> bool {
    if name.chars().count() > MAX_VARIABLE_NAME_CHARS {
        return false;
    }
    let mut characters = name.chars();
    match characters.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// Выполняет подстановки без повторного раскрытия вставленных значений.
fn expand(
    source: &str,
    values: &HashMap<String, String>,
    captures: &[String],
) -> Result<String, VariableError> {
    let mut output = String::with_capacity(source.len());
    let mut output_chars = 0_usize;
    let mut remaining = source;
    while let Some(marker) = remaining.find("${") {
        let Some(prefix) = remaining.get(..marker) else {
            push_bounded(&mut output, &mut output_chars, remaining)?;
            return Ok(output);
        };
        push_bounded(&mut output, &mut output_chars, prefix)?;
        let Some(literal_reference) = remaining.get(marker..) else {
            return Ok(output);
        };
        let Some(reference_start) = marker.checked_add(2) else {
            push_bounded(&mut output, &mut output_chars, remaining)?;
            return Ok(output);
        };
        let Some(after_marker) = remaining.get(reference_start..) else {
            push_bounded(&mut output, &mut output_chars, remaining)?;
            return Ok(output);
        };
        let Some(end) = after_marker.find('}') else {
            push_bounded(&mut output, &mut output_chars, literal_reference)?;
            return Ok(output);
        };
        let Some(reference) = after_marker.get(..end) else {
            push_bounded(&mut output, &mut output_chars, literal_reference)?;
            return Ok(output);
        };
        match resolve(reference, values, captures)? {
            Some(value) => push_bounded(&mut output, &mut output_chars, value)?,
            None => {
                // Сохраняем некорректный префикс, затем ищем вложенную ссылку.
                push_bounded(&mut output, &mut output_chars, "${")?;
                remaining = after_marker;
                continue;
            }
        }
        let Some(next_start) = end.checked_add(1) else {
            return Ok(output);
        };
        let Some(next) = after_marker.get(next_start..) else {
            return Ok(output);
        };
        remaining = next;
    }
    push_bounded(&mut output, &mut output_chars, remaining)?;
    Ok(output)
}

/// Дописывает Unicode scalar values в пределах локального expansion budget.
fn push_bounded(
    output: &mut String,
    output_chars: &mut usize,
    value: &str,
) -> Result<(), VariableError> {
    let next_count = output_chars.saturating_add(value.chars().count());
    if next_count > MAX_EXPANDED_STRING_CHARS {
        return Err(VariableError::ExpansionLimit);
    }
    output.push_str(value);
    *output_chars = next_count;
    Ok(())
}

/// Возвращает `Some("")` для корректной, но неизвестной ссылки.
fn resolve<'a>(
    reference: &str,
    values: &'a HashMap<String, String>,
    captures: &'a [String],
) -> Result<Option<&'a str>, VariableError> {
    if reference.is_empty() {
        return Ok(None);
    }
    if reference.bytes().all(|byte| byte.is_ascii_digit()) {
        let index = reference
            .parse::<usize>()
            .map_err(|_| VariableError::UnsupportedCaptureIndex)?;
        if index > 9 {
            return Err(VariableError::UnsupportedCaptureIndex);
        }
        return Ok(Some(captures.get(index).map(String::as_str).unwrap_or("")));
    }
    if !valid_name(reference) {
        return match valid_namespace(reference) {
            true => Err(VariableError::UnsupportedNamespace),
            false => Ok(None),
        };
    }
    Ok(Some(
        values
            .get(&reference.to_ascii_lowercase())
            .map(String::as_str)
            .unwrap_or(""),
    ))
}

/// Отличает зарезервированный namespace syntax от просто некорректной ссылки.
fn valid_namespace(reference: &str) -> bool {
    let mut components = reference.split('.');
    let Some(first) = components.next() else {
        return false;
    };
    if !valid_name(first) {
        return false;
    }
    let mut count = 1_usize;
    for component in components {
        if component.is_empty() || !valid_variable_component(component) {
            return false;
        }
        count = count.saturating_add(1);
    }
    count >= 2
}

/// Проверяет обычный identifier либо decimal match-variable component.
fn valid_variable_component(value: &str) -> bool {
    match value.bytes().all(|byte| byte.is_ascii_digit()) {
        true => !value.is_empty(),
        false => valid_name(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_ascii_case_insensitive_and_scoped_to_store() {
        let mut first = VariableStore::default();
        let second = VariableStore::default();
        first.set("Folder", "Invoices").expect("valid name");

        assert_eq!(first.get("FOLDER"), Some("Invoices"));
        assert_eq!(second.get("folder"), None);
    }

    #[test]
    fn expansion_is_single_pass_and_unknown_reference_is_empty() {
        let mut store = VariableStore::default();
        store.set("word", "${missing}").expect("valid name");

        assert_eq!(
            store.expand("${WORD}/${other}", &[]).expect("bounded"),
            "${missing}/"
        );
    }

    #[test]
    fn malformed_reference_stays_literal_and_nested_valid_reference_expands() {
        let mut store = VariableStore::default();
        store.set("company", "ACME").expect("valid name");

        assert_eq!(
            store
                .expand("${doh!} ${foo.bar!} ${BAD${Company}", &[])
                .expect("bounded"),
            "${doh!} ${foo.bar!} ${BADACME"
        );
    }

    #[test]
    fn numeric_references_select_captures_and_out_of_range_is_empty() {
        let store = VariableStore::default();
        let captures = vec!["whole".to_owned(), "list".to_owned()];

        assert_eq!(
            store.expand("${0}:${01}:${9}", &captures).expect("bounded"),
            "whole:list:"
        );
        assert_eq!(
            store.expand("${10}", &captures),
            Err(VariableError::UnsupportedCaptureIndex)
        );
        assert_eq!(
            store.expand("${vendor.value}", &captures),
            Err(VariableError::UnsupportedNamespace)
        );
    }

    #[test]
    fn expansion_rejects_output_above_the_explicit_runtime_budget() {
        let mut store = VariableStore::default();
        store
            .set("wide", &"x".repeat(MAX_VARIABLE_VALUE_CHARS))
            .expect("valid name");
        let source = "${wide}".repeat(17);

        assert_eq!(
            store.expand(&source, &[]),
            Err(VariableError::ExpansionLimit)
        );
    }

    #[test]
    fn store_enforces_name_and_count_limits_and_truncates_by_unicode_character() {
        let mut store = VariableStore::default();
        assert_eq!(store.set("9bad", "x"), Err(VariableError::InvalidName));
        let long_value = "я".repeat(MAX_VARIABLE_VALUE_CHARS + 3);
        store.set("v0", &long_value).expect("valid name");
        assert_eq!(
            store.get("V0").map(str::chars).map(Iterator::count),
            Some(4_000)
        );
        for number in 1..MAX_VARIABLES {
            store
                .set(&format!("v{number}"), "x")
                .expect("within variable limit");
        }
        assert_eq!(
            store.set("overflow", "x"),
            Err(VariableError::TooManyVariables)
        );
    }
}
