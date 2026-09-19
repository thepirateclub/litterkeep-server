//! Узкая Linux FFI-граница для advisory writer lock.
#![allow(unsafe_code)]
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
const LOCK_EX: i32 = 2;
const LOCK_NB: i32 = 4;
unsafe extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}
/// Захватывает исключительный неблокирующий lock до закрытия file descriptor.
pub fn acquire(file: &File) -> io::Result<()> {
    // SAFETY: fd принадлежит live File, flock не получает владение дескриптором.
    let result = unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) };
    match result {
        0 => Ok(()),
        _ => Err(io::Error::last_os_error()),
    }
}
