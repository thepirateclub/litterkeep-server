//! Minimal `poll(2)` wrapper. This is the only unsafe boundary in the crate.
#![allow(unsafe_code)]

use std::io;
use std::os::fd::BorrowedFd;

const POLLIN: i16 = 0x0001;
const POLLOUT: i16 = 0x0004;
const POLLERR: i16 = 0x0008;
const POLLHUP: i16 = 0x0010;
const POLLNVAL: i16 = 0x0020;
const EINTR: i32 = 4;

#[repr(C)]
struct RawPollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

unsafe extern "C" {
    fn poll(fds: *mut RawPollFd, nfds: usize, timeout: i32) -> i32;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Interest {
    pub readable: bool,
    pub writable: bool,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Ready {
    pub readable: bool,
    pub writable: bool,
    pub closed_or_error: bool,
}

pub fn wait(fds: &[(BorrowedFd<'_>, Interest)], timeout_ms: i32) -> io::Result<Vec<Ready>> {
    let mut raw: Vec<RawPollFd> = fds
        .iter()
        .map(|(fd, interest)| RawPollFd {
            fd: fd.as_raw_fd(),
            events: (if interest.readable { POLLIN } else { 0 })
                | (if interest.writable { POLLOUT } else { 0 }),
            revents: 0,
        })
        .collect();
    let timeout = timeout_ms.max(-1);
    loop {
        // SAFETY: `raw` is live, contiguous `repr(C)` storage for exactly its
        // length. poll borrows descriptors only for this call and takes no ownership.
        let result = unsafe { poll(raw.as_mut_ptr(), raw.len(), timeout) };
        if result >= 0 {
            return Ok(raw
                .into_iter()
                .map(|fd| Ready {
                    readable: fd.revents & POLLIN != 0,
                    writable: fd.revents & POLLOUT != 0,
                    closed_or_error: fd.revents & (POLLERR | POLLHUP | POLLNVAL) != 0,
                })
                .collect());
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(EINTR) {
            return Err(error);
        }
    }
}

use std::os::fd::AsRawFd;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    #[test]
    fn reports_readable_socket() {
        let (mut writer, reader) = UnixStream::pair().expect("pair");
        writer.write_all(b"w").expect("write");
        let ready = wait(
            &[(
                reader.as_fd(),
                Interest {
                    readable: true,
                    writable: false,
                },
            )],
            1000,
        )
        .expect("poll");
        assert!(ready[0].readable);
    }
}
