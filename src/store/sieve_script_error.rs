//! Типизированные причины отказа операций над каталогом ManageSieve.

use std::io;

/// Причина хранится в `io::Error` как source для передачи через worker boundary.
#[derive(Debug)]
enum ScriptStoreError {
    Quota(&'static str),
    Missing,
    Active,
}

impl std::fmt::Display for ScriptStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Quota(message) => formatter.write_str(message),
            Self::Missing => formatter.write_str("sieve script missing"),
            Self::Active => formatter.write_str("active sieve script cannot be deleted"),
        }
    }
}

impl std::error::Error for ScriptStoreError {}

/// Формирует только собственную квотную ошибку Store, не filesystem ENOSPC.
pub(super) fn quota(message: &'static str) -> io::Error {
    io::Error::other(ScriptStoreError::Quota(message))
}

/// Сохраняет привычный NotFound kind и помечает причину для RFC ответа.
pub(super) fn missing() -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, ScriptStoreError::Missing)
}

/// Помечает отказ удаления active script отдельным machine-readable source.
pub(super) fn active() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, ScriptStoreError::Active)
}

#[cfg(any(test, target_os = "linux"))]
pub(super) fn is_quota(error: &io::Error) -> bool {
    matches_source(error, |cause| matches!(cause, ScriptStoreError::Quota(_)))
}

#[cfg(any(test, target_os = "linux"))]
pub(super) fn is_missing(error: &io::Error) -> bool {
    matches_source(error, |cause| matches!(cause, ScriptStoreError::Missing))
}

#[cfg(any(test, target_os = "linux"))]
pub(super) fn is_active(error: &io::Error) -> bool {
    matches_source(error, |cause| matches!(cause, ScriptStoreError::Active))
}

#[cfg(any(test, target_os = "linux"))]
fn matches_source(error: &io::Error, matches: impl FnOnce(&ScriptStoreError) -> bool) -> bool {
    error
        .get_ref()
        .and_then(|source| source.downcast_ref::<ScriptStoreError>())
        .is_some_and(matches)
}
