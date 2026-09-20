//! Offline maintenance, использующий те же форматы и exclusive Store lock.

#[path = "maintenance/backup.rs"]
mod backup;
#[path = "maintenance/check.rs"]
mod check;

pub use backup::{BackupReport, backup};
pub use check::{CheckReport, check};

#[cfg(test)]
#[path = "maintenance/check_tests.rs"]
mod check_tests;

#[cfg(test)]
#[path = "maintenance/backup_tests.rs"]
mod backup_tests;
