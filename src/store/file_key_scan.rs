//! Перечисление legacy- и hex-кодированных файловых ключей.

use super::FileKind;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Декодирует ключи, не следуя по символическим ссылкам.
pub(super) fn entries(root: &Path, kind: FileKind) -> io::Result<Vec<(String, PathBuf)>> {
    let mut result = Vec::new();
    read_legacy(root, kind, &mut result)?;
    let encoded = root.join(super::ENCODED_DIRECTORY);
    match fs::symlink_metadata(&encoded) {
        Ok(metadata) if metadata.is_dir() => {
            walk_encoded(&encoded, kind, 0, String::new(), &mut result)?
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(_) => {
            return Err(super::invalid(
                "encoded mailbox namespace is not a directory",
            ));
        }
        Err(error) => return Err(error),
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
            return Err(super::invalid("mailbox key is not a regular file"));
        }
        let stem = path.file_stem().and_then(|value| value.to_str());
        let stem = stem.ok_or_else(|| super::invalid("non-utf8 mailbox file key"))?;
        let key = stem.to_owned();
        super::super::model::validate_mailbox(&key)?;
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
    if depth > super::MAX_ENCODED_DIRECTORY_DEPTH {
        return Err(super::invalid(
            "encoded mailbox key exceeds directory depth",
        ));
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(super::invalid("mailbox key must not be a symbolic link"));
        }
        match file_type.is_dir() {
            true => {
                let component = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| super::invalid("non-utf8 mailbox path component"))?;
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
            false => Err(super::invalid("mailbox key is not a regular file")),
        };
    }
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(super::invalid("mailbox key is not a regular file"));
    }
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| super::invalid("non-utf8 mailbox file key"))?;
    let key = super::codec::decode_hex(&format!("{prefix}{stem}"))?;
    super::super::model::validate_mailbox(&key)?;
    result.push((key, path.to_path_buf()));
    Ok(())
}

fn validate_chunk(value: &str) -> io::Result<()> {
    let is_valid = value.len() == super::HEX_DIRECTORY_CHUNK
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
    match is_valid {
        true => Ok(()),
        false => Err(super::invalid("invalid encoded mailbox directory")),
    }
}
