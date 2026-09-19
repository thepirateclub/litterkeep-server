//! `litterkeep-server` is deliberately built from small, owned components.
//! The first component is the lifecycle orchestrator: protocol code may not
//! hand an exclusive resource to another task without going through it.

#![deny(unsafe_code)]

pub mod accounts;
pub mod auth;
pub mod imap;
pub mod mime;
pub mod outbound;
pub mod platform;
pub mod pop3;
pub mod queue;
#[cfg(target_os = "linux")]
pub mod reactor;
pub mod runtime;
pub mod sasl;
pub mod sieve;
pub mod smtp;
pub mod storage_worker;
pub mod store;
pub mod tls;
pub mod tls_transport;

pub const PRODUCT: &str = "litterkeep-server";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
