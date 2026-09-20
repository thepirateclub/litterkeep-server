//! Nonblocking Unix-domain listener и короткие control connections.

use super::{Command, response};
use crate::runtime::Orchestrator;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::{
    fs::{FileTypeExt, PermissionsExt},
    net::{UnixListener, UnixStream},
};
use std::path::{Path, PathBuf};

const MAX_REQUEST: usize = 256;
const ACCEPT_BUDGET: usize = 8;

/// Владелец local socket, размещённого внутри data-dir сервера.
pub struct Listener {
    listener: UnixListener,
    path: PathBuf,
    locator: Option<PathBuf>,
}

impl Listener {
    /// Создаёт сокет с правами только владельца; stale socket удаляется лишь если это socket.
    pub fn bind(root: impl AsRef<Path>) -> io::Result<Self> {
        let path = root.as_ref().join("runtime.sock");
        match bind_socket(&path) {
            Ok(listener) => Ok(Self {
                listener,
                path,
                locator: None,
            }),
            Err(error) if supports_fallback(&error) => {
                remove_stale_socket(&path)?;
                bind_temporary_socket(root.as_ref())
            }
            Err(error) => Err(error),
        }
    }

    /// Descriptor контролируется тем же poll-loop, что и почтовые listener-ы.
    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.listener.as_fd()
    }

    /// Принимает ограниченное число клиентов, не создавая поток на диагностику.
    pub fn accept(&self, sessions: &mut Vec<Option<Connection>>) -> io::Result<()> {
        for _ in 0..ACCEPT_BUDGET {
            match self.listener.accept() {
                Ok((stream, _)) => place(sessions, Connection::new(stream)?),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        if let Some(locator) = self.locator.as_ref() {
            let _ = std::fs::remove_file(locator);
        }
    }
}

/// Одна короткая команда, которую reactor обслуживает по readable/writable readiness.
pub struct Connection {
    stream: UnixStream,
    input: Vec<u8>,
    output: Vec<u8>,
}

/// Итог одного readiness-step без неявного закрытия socket-а.
pub enum ServiceResult {
    Keep,
    Close,
}

impl Connection {
    fn new(stream: UnixStream) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            input: Vec::new(),
            output: Vec::new(),
        })
    }

    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }

    pub fn wants_write(&self) -> bool {
        !self.output.is_empty()
    }

    pub fn service<T>(
        &mut self,
        readable: bool,
        writable: bool,
        lifecycle: &Orchestrator<T>,
    ) -> io::Result<ServiceResult> {
        if readable {
            self.read_request(lifecycle)?;
        }
        if writable {
            self.write_response()?;
        }
        match (self.output.is_empty(), self.input.is_empty()) {
            (true, false) => Ok(ServiceResult::Close),
            (true, true) | (false, _) => Ok(ServiceResult::Keep),
        }
    }

    fn read_request<T>(&mut self, lifecycle: &Orchestrator<T>) -> io::Result<()> {
        let mut chunk = [0_u8; 128];
        loop {
            match self.stream.read(&mut chunk) {
                Ok(0) => return Ok(()),
                Ok(count) => {
                    let Some(received) = chunk.get(..count) else {
                        return Err(io::Error::other("invalid control read length"));
                    };
                    self.input.extend_from_slice(received);
                    if self.input.len() > MAX_REQUEST {
                        self.output = b"ERROR request too long\n".to_vec();
                        return Ok(());
                    }
                    if self.input.contains(&b'\n') {
                        self.output = parse_response(lifecycle, &self.input);
                        return Ok(());
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) => return Err(error),
            }
        }
    }

    fn write_response(&mut self) -> io::Result<()> {
        match self.stream.write(&self.output) {
            Ok(count) => {
                self.output.drain(..count);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error),
        }
        Ok(())
    }
}

fn parse_response<T>(lifecycle: &Orchestrator<T>, bytes: &[u8]) -> Vec<u8> {
    let line = std::str::from_utf8(bytes).unwrap_or_default();
    match Command::parse(line) {
        Ok(command) => response(lifecycle, command).into_bytes(),
        Err(error) => format!("ERROR {error}\n").into_bytes(),
    }
}

fn place(sessions: &mut Vec<Option<Connection>>, connection: Connection) {
    match sessions.iter_mut().find(|entry| entry.is_none()) {
        Some(slot) => *slot = Some(connection),
        None => sessions.push(Some(connection)),
    }
}

fn remove_stale_socket(path: &Path) -> io::Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_socket() {
        return std::fs::remove_file(path);
    }
    if metadata.len() == 0 {
        // Shared folder Docker Desktop материализует неудачный Unix bind
        // нулевым placeholder. Пока удерживается Store lock, `runtime.sock`
        // принадлежит только серверу, поэтому пользовательский файл не затронут.
        return std::fs::remove_file(path);
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "control path is not a socket",
    ))
}

/// Связанные с Windows shared volume файловые системы могут не поддерживать socket inode.
fn bind_temporary_socket(root: &Path) -> io::Result<Listener> {
    let path = std::env::temp_dir().join(format!("litterkeep-control-{}.sock", std::process::id()));
    let listener = bind_socket(&path)?;
    let locator = root.join("runtime.control");
    let write_result = std::fs::write(&locator, path.as_os_str().as_encoded_bytes());
    if let Err(error) = write_result {
        drop(listener);
        let _ = std::fs::remove_file(&path);
        return Err(error);
    }
    std::fs::set_permissions(&locator, std::fs::Permissions::from_mode(0o600))?;
    Ok(Listener {
        listener,
        path,
        locator: Some(locator),
    })
}

/// Создаёт socket и сразу ограничивает filesystem permissions до owner-only.
fn bind_socket(path: &Path) -> io::Result<UnixListener> {
    remove_stale_socket(path)?;
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

/// Не скрывает обычные ошибки binding-а за fallback, кроме известных non-socket mount-ов.
fn supports_fallback(error: &io::Error) -> bool {
    match error.kind() {
        io::ErrorKind::Unsupported | io::ErrorKind::PermissionDenied => true,
        _ => matches!(error.raw_os_error(), Some(5)),
    }
}
