//! Создание и регистрация TCP listener-ов в общем poll-наборе.

use super::Target;
use crate::platform::linux_poll::Interest;
use std::io;
use std::net::TcpListener;
use std::os::fd::{AsFd, BorrowedFd};

/// Создаёт только явно запрошенный nonblocking listener.
pub(super) fn bind(address: Option<&str>) -> io::Result<Option<TcpListener>> {
    let Some(address) = address else {
        return Ok(None);
    };
    let listener = TcpListener::bind(address)?;
    listener.set_nonblocking(true)?;
    Ok(Some(listener))
}

/// Сохраняет одинаковый порядок descriptor-ов и typed target-ов.
pub(super) fn append<'a>(
    listener: Option<&'a TcpListener>,
    target: Target,
    targets: &mut Vec<Target>,
    poll_fds: &mut Vec<(BorrowedFd<'a>, Interest)>,
) {
    let Some(listener) = listener else {
        return;
    };
    targets.push(target);
    poll_fds.push((
        listener.as_fd(),
        Interest {
            readable: true,
            writable: false,
        },
    ));
}
