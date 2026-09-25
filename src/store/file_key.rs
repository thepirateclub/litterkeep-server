//! Безопасное отображение логических mailbox-ключей на имена файлов.
//!
//! Старые ASCII ключи сохраняют прежние пути. Остальные ключи хранятся в
//! отдельном каталоге с hex-представлением UTF-8, поэтому символы протокола
//! никогда не становятся компонентами системного пути.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const ENCODED_DIRECTORY: &str = "utf8";
const HEX_DIRECTORY_CHUNK: usize = 120;
const MAX_ENCODED_DIRECTORY_DEPTH: usize = 4;

/// Разделяет семантические виды mailbox-файлов до работы с дисковым путём.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FileKind {
    Journal,
    Flags,
    Expunges,
}

impl FileKind {
    fn extension(self) -> &'static str {
        match self {
            Self::Journal => "journal",
            Self::Flags => "flags",
            Self::Expunges => "expunges",
        }
    }
}

/// Возвращает устойчивый путь файла для ключа и расширения.
pub(super) fn path(root: &Path, key: &str, kind: FileKind) -> PathBuf {
    let extension = kind.extension();
    match legacy_key(key) {
        true => root.join(format!("{key}.{extension}")),
        false => encoded_path(root, key, extension),
    }
}

fn encoded_path(root: &Path, key: &str, extension: &str) -> PathBuf {
    let mut directory = root.join(ENCODED_DIRECTORY);
    let mut remaining = encode_hex(key.as_bytes());
    while remaining.len() > HEX_DIRECTORY_CHUNK {
        let Some((chunk, tail)) = remaining.split_at_checked(HEX_DIRECTORY_CHUNK) else {
            break;
        };
        directory.push(chunk);
        remaining = tail.to_owned();
    }
    directory.join(format!("{remaining}.{extension}"))
}

/// Создаёт кодированное пространство имён и сохраняет его запись в родителе.
pub(super) fn prepare(root: &Path, key: &str, kind: FileKind) -> io::Result<PathBuf> {
    let path = path(root, key, kind);
    let Some(parent) = path.parent() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing key parent",
        ));
    };
    if parent != root {
        ensure_encoded_directory(root, parent)?;
    }
    Ok(path)
}

fn ensure_encoded_directory(root: &Path, directory: &Path) -> io::Result<()> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(invalid("encoded mailbox namespace is not a directory")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => match fs::create_dir(directory) {
            Ok(()) => super::durability::sync_directory(root),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                match fs::symlink_metadata(directory) {
                    Ok(metadata) if metadata.is_dir() => Ok(()),
                    Ok(_) => Err(invalid("encoded mailbox namespace is not a directory")),
                    Err(metadata_error) => Err(metadata_error),
                }
            }
            Err(error) => Err(error),
        },
        Err(error) => Err(error),
    }
}

/// Читает файлы обоих пространств имён без изменения старых данных.
pub(super) fn entries(root: &Path, kind: FileKind) -> io::Result<Vec<(String, PathBuf)>> {
    let mut result = read_directory(root, kind, false)?;
    let encoded = root.join(ENCODED_DIRECTORY);
    match fs::symlink_metadata(&encoded) {
        Ok(metadata) if metadata.is_dir() => result.extend(read_directory(&encoded, kind, true)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(_) => return Err(invalid("encoded mailbox namespace is not a directory")),
        Err(error) => return Err(error),
    }
    Ok(result)
}

fn read_directory(
    directory: &Path,
    kind: FileKind,
    encoded: bool,
) -> io::Result<Vec<(String, PathBuf)>> {
    let mut result = Vec::new();
    match encoded {
        true => walk_encoded(directory, kind, 0, String::new(), &mut result)?,
        false => read_legacy(directory, kind, &mut result)?,
    }
    Ok(result)
}

fn read_legacy(
    directory: &Path,
    kind: FileKind,
    result: &mut Vec<(String, PathBuf)>,
) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) != Some(kind.extension()) {
            continue;
        }
        if !fs::symlink_metadata(&path)?.file_type().is_file() {
            return Err(invalid("mailbox key is not a regular file"));
        }
        let stem = path.file_stem().and_then(|value| value.to_str());
        let stem = stem.ok_or_else(|| invalid("non-utf8 mailbox file key"))?;
        let key = stem.to_owned();
        super::model::validate_mailbox(&key)?;
        result.push((key, path));
    }
    Ok(())
}

fn walk_encoded(
    directory: &Path,
    kind: FileKind,
    depth: usize,
    prefix: String,
    result: &mut Vec<(String, PathBuf)>,
) -> io::Result<()> {
    if depth > MAX_ENCODED_DIRECTORY_DEPTH {
        return Err(invalid("encoded mailbox key exceeds directory depth"));
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(invalid("mailbox key must not be a symbolic link"));
        }
        match file_type.is_dir() {
            true => {
                let component = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| invalid("non-utf8 mailbox path component"))?;
                validate_chunk(&component)?;
                let next_prefix = format!("{prefix}{component}");
                walk_encoded(&path, kind, depth + 1, next_prefix, result)?;
            }
            false => read_encoded_file(&path, kind, &prefix, result)?,
        }
    }
    Ok(())
}

fn read_encoded_file(
    path: &Path,
    kind: FileKind,
    prefix: &str,
    result: &mut Vec<(String, PathBuf)>,
) -> io::Result<()> {
    let has_expected_extension =
        path.extension().and_then(|value| value.to_str()) == Some(kind.extension());
    if !has_expected_extension {
        return match path.is_file() {
            true => Ok(()),
            false => Err(invalid("mailbox key is not a regular file")),
        };
    }
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(invalid("mailbox key is not a regular file"));
    }
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("non-utf8 mailbox file key"))?;
    let key = decode_hex(&format!("{prefix}{stem}"))?;
    super::model::validate_mailbox(&key)?;
    result.push((key, path.to_path_buf()));
    Ok(())
}

fn validate_chunk(value: &str) -> io::Result<()> {
    let is_valid = value.len() == HEX_DIRECTORY_CHUNK
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
    match is_valid {
        true => Ok(()),
        false => Err(invalid("invalid encoded mailbox directory")),
    }
}

fn legacy_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        let high = usize::from(byte >> 4);
        let low = usize::from(byte & 0x0f);
        if let (Some(high), Some(low)) = (DIGITS.get(high), DIGITS.get(low)) {
            encoded.push(char::from(*high));
            encoded.push(char::from(*low));
        }
    }
    encoded
}

fn decode_hex(value: &str) -> io::Result<String> {
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let mut octets = value.as_bytes().chunks_exact(2);
    for pair in &mut octets {
        let high = pair.first().and_then(|byte| hex_value(*byte));
        let low = pair.get(1).and_then(|byte| hex_value(*byte));
        match (high, low) {
            (Some(high), Some(low)) => bytes.push((high << 4) | low),
            _ => return Err(invalid("invalid encoded mailbox key")),
        }
    }
    if !octets.remainder().is_empty() {
        return Err(invalid("odd encoded mailbox key"));
    }
    String::from_utf8(bytes).map_err(|_| invalid("encoded mailbox key is not utf8"))
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
#[path = "file_key_tests.rs"]
mod tests;
