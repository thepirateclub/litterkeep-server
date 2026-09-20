//! CLI-клиент защищённого local control socket без второго Store writer-а.

use crate::usage;
use std::env;
use std::process::ExitCode;

/// Выполняет одну read-only runtime-команду через socket уже работающего сервера.
pub(crate) fn run() -> ExitCode {
    let (request, data_dir) = match arguments() {
        Ok(value) => value,
        Err(code) => return code,
    };
    #[cfg(target_os = "linux")]
    return request_linux(&data_dir, &request);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (request, data_dir);
        eprintln!("runtime control socket is available only on Linux");
        ExitCode::FAILURE
    }
}

/// Поддерживает documented form `runtime <command> ... --data-dir <dir>`.
fn arguments() -> Result<(String, String), ExitCode> {
    let mut values = env::args().skip(2);
    let Some(action) = values.next() else {
        usage();
        return Err(ExitCode::from(2));
    };
    let request = match action.as_str() {
        "stats" | "tasks" | "resources" => action,
        "inspect" | "cancel" => {
            let Some(task_id) = values.next() else {
                usage();
                return Err(ExitCode::from(2));
            };
            format!("{action} {task_id}")
        }
        _ => {
            usage();
            return Err(ExitCode::from(2));
        }
    };
    let Some(option) = values.next() else {
        usage();
        return Err(ExitCode::from(2));
    };
    let Some(data_dir) = values.next() else {
        usage();
        return Err(ExitCode::from(2));
    };
    match (option.as_str(), values.next()) {
        ("--data-dir", None) => Ok((request, data_dir)),
        _ => {
            usage();
            Err(ExitCode::from(2))
        }
    }
}

/// Unix-domain transport не раскрывает control protocol в сетевом listener-е.
#[cfg(target_os = "linux")]
fn request_linux(data_dir: &str, request: &str) -> ExitCode {
    use std::io::{Read, Write};
    use std::net::Shutdown;
    use std::os::unix::net::UnixStream;
    let path = match socket_path(data_dir) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("runtime control locator error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut stream = match UnixStream::connect(path) {
        Ok(stream) => stream,
        Err(error) => {
            eprintln!("runtime control connection error: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = writeln!(stream, "{request}") {
        eprintln!("runtime control write error: {error}");
        return ExitCode::FAILURE;
    }
    if let Err(error) = stream.shutdown(Shutdown::Write) {
        eprintln!("runtime control shutdown error: {error}");
        return ExitCode::FAILURE;
    }
    let mut response = String::new();
    match stream.read_to_string(&mut response) {
        Ok(_) => {
            print!("{response}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("runtime control read error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// На обычном Linux socket лежит в data-dir; shared volume получает защищённый locator.
#[cfg(target_os = "linux")]
fn socket_path(data_dir: &str) -> std::io::Result<std::path::PathBuf> {
    let root = std::path::Path::new(data_dir);
    let direct = root.join("runtime.sock");
    if direct.exists() {
        return Ok(direct);
    }
    let value = std::fs::read_to_string(root.join("runtime.control"))?;
    let path = std::path::PathBuf::from(value.trim());
    if path.starts_with(std::env::temp_dir()) {
        return Ok(path);
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "runtime control locator escapes temporary directory",
    ))
}
