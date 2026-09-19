//! Narrow platform boundaries used by the reactor.

#[cfg(target_os = "linux")]
#[path = "platform/linux_lock.rs"]
pub mod linux_lock;
#[cfg(target_os = "linux")]
#[path = "platform/linux_poll.rs"]
pub mod linux_poll;
