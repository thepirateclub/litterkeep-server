//! Проверка и resolution будущих Sieve namespaces из RFC 5229.

use super::super::keywords::{CapabilityWord, CapabilityWord::Variables};
use super::VariableError;

/// Проверяет все syntactically valid variable refs, включая вложенные формы.
pub(super) fn validate_references(source: &str) -> Result<(), VariableError> {
    let mut remaining = source;
    while let Some(marker) = remaining.find("${") {
        let Some(start) = marker.checked_add(2) else {
            return Ok(());
        };
        let Some(after_start) = remaining.get(start..) else {
            return Ok(());
        };
        let Some(end) = after_start.find('}') else {
            return Ok(());
        };
        let Some(reference) = after_start.get(..end) else {
            return Ok(());
        };
        if namespaced(reference) && !supported_namespace(reference) {
            return Err(VariableError::UnsupportedNamespace);
        }
        let Some(next_start) = start.checked_add(1) else {
            return Ok(());
        };
        let Some(next) = remaining.get(next_start..) else {
            return Ok(());
        };
        remaining = next;
    }
    Ok(())
}

/// Известный, но пока не поставляющий значения namespace даёт empty string.
pub(super) fn resolve(reference: &str) -> Result<Option<&'static str>, VariableError> {
    if supported_namespace(reference) {
        return Ok(Some(""));
    }
    Err(VariableError::UnsupportedNamespace)
}

fn namespaced(reference: &str) -> bool {
    super::valid_namespace(reference)
}

fn supported_namespace(reference: &str) -> bool {
    match reference.split_once('.') {
        Some((namespace, _)) => matches!(CapabilityWord::parse(namespace), Variables),
        None => false,
    }
}
