//! Ограниченное хранилище и однопроходное раскрытие Sieve Variables.

use std::collections::HashMap;

#[path = "variables/modifier.rs"]
mod modifier;
pub use modifier::VariableModifier;

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
    UnsupportedNamespace,
    InvalidModifiers,
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

    /// Применяет RFC 5229 modifiers до записи значения в переменную.
    pub fn set_modified(
        &mut self,
        name: &str,
        value: &str,
        modifiers: &[VariableModifier],
    ) -> Result<(), VariableError> {
        let modified = modifier::apply(value, modifiers)?;
        self.set(name, &modified)
    }

    /// Раскрывает текущие ссылки, применяет modifiers и лишь затем сохраняет.
    pub fn set_expanded(
        &mut self,
        name: &str,
        source: &str,
        captures: &[String],
        modifiers: &[VariableModifier],
    ) -> Result<(), VariableError> {
        if !valid_name(name) {
            return Err(VariableError::InvalidName);
        }
        let expanded = self.expand(source, captures)?;
        self.set_modified(name, &expanded, modifiers)
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
        let index = reference.parse::<usize>().unwrap_or(usize::MAX);
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
#[path = "variables_tests.rs"]
mod tests;
