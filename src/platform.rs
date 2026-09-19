//! Narrow platform boundaries used by the reactor.

#[cfg(target_os = "linux")]
#[path = "platform/linux_poll.rs"]
pub mod linux_poll;
