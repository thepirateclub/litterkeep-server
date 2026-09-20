//! Идемпотентное сетевое наполнение тестовых mailbox-ов.

#[path = "seed/fixture.rs"]
mod fixture;
#[path = "seed/imap_client.rs"]
mod imap_client;
#[path = "seed/runner.rs"]
mod runner;

pub use runner::{SeedConfig, SeedReport, run};
