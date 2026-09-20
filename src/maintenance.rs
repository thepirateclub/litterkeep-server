//! Offline maintenance, использующий те же форматы и exclusive Store lock.

#[path = "maintenance/check.rs"]
mod check;

pub use check::{CheckReport, check};

#[cfg(test)]
#[path = "maintenance/check_tests.rs"]
mod check_tests;
