//! Детерминированный RFC 5230 response identity без хранения содержимого.

use super::ast::VacationAction;
use argon2::{Algorithm, Argon2, Params, Version};
use std::io;

const IDENTITY_SALT: [u8; 16] = *b"LitterkeepVacay1";

/// Хеширует handle либо специфицированные RFC аргументы в фиксированные 32 байта.
pub(crate) fn response_identity(action: &VacationAction) -> io::Result<String> {
    let mut input = Vec::new();
    match action.handle.as_deref() {
        Some(handle) => {
            input.push(1);
            field(&mut input, Some(handle.as_bytes()))?;
        }
        None => {
            input.push(0);
            field(&mut input, action.subject.as_deref().map(str::as_bytes))?;
            field(&mut input, action.from.as_deref().map(str::as_bytes))?;
            input.push(u8::from(action.mime));
            field(&mut input, Some(&action.reason))?;
        }
    }
    let parameters = Params::new(8, 1, 1, Some(32))
        .map_err(|_| io::Error::other("vacation identity parameters invalid"))?;
    let hasher = Argon2::new(Algorithm::Argon2id, Version::V0x13, parameters);
    let mut digest = [0_u8; 32];
    hasher
        .hash_password_into(&input, &IDENTITY_SALT, &mut digest)
        .map_err(|_| io::Error::other("vacation identity hash failed"))?;
    Ok(hex(&digest))
}

fn field(target: &mut Vec<u8>, value: Option<&[u8]>) -> io::Result<()> {
    let (present, bytes) = match value {
        Some(bytes) => (1_u8, bytes),
        None => (0_u8, &[] as &[u8]),
    };
    let length = u32::try_from(bytes.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "vacation identity too long"))?;
    target.push(present);
    target.extend_from_slice(&length.to_be_bytes());
    target.extend_from_slice(bytes);
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut value, "{byte:02x}").expect("String formatting cannot fail");
    }
    value
}
