//! Разбор initial response механизма SASL PLAIN без готового SASL framework.
//!
//! Модуль не проверяет пароль. Он формирует типизированные credentials, которые
//! затем передаются единственному владельцу account-хранилища.
#[path = "sasl/hmac_sha1.rs"]
pub mod hmac_sha1;
#[path = "sasl/pbkdf2_sha1.rs"]
pub mod pbkdf2_sha1;
#[path = "sasl/saslprep.rs"]
pub mod saslprep;
#[path = "sasl/saslprep_data/data.rs"]
mod saslprep_data;
#[path = "sasl/scram_attributes.rs"]
pub mod scram_attributes;
#[path = "sasl/scram_sha1_record.rs"]
pub mod scram_sha1_record;
#[path = "sasl/scram_sha1_verifier.rs"]
pub mod scram_sha1_verifier;
#[path = "sasl/sha1.rs"]
pub mod sha1;

use crate::mime::decode_base64;

/// Проверенные поля PLAIN без исходного base64-представления.
#[derive(Debug, Eq, PartialEq)]
pub struct PlainCredentials {
    pub account: String,
    pub password: Vec<u8>,
}

/// Кодирует клиентский SASL PLAIN initial response без стороннего framework-а.
pub fn encode_plain(account: &str, password: &[u8]) -> String {
    let mut payload = Vec::with_capacity(
        account
            .len()
            .saturating_add(password.len())
            .saturating_add(2),
    );
    payload.push(0);
    payload.extend_from_slice(account.as_bytes());
    payload.push(0);
    payload.extend_from_slice(password);
    encode_base64(&payload)
}

/// Строит стандартный base64 без переноса строк для SASL command argument.
fn encode_base64(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(input.len().div_ceil(3).saturating_mul(4));
    for chunk in input.chunks(3) {
        let first = u32::from(chunk.first().copied().unwrap_or(0));
        let second = u32::from(chunk.get(1).copied().unwrap_or(0));
        let third = u32::from(chunk.get(2).copied().unwrap_or(0));
        let value = (first << 16) | (second << 8) | third;
        for shift in [18_u32, 12, 6, 0] {
            let index = usize::try_from((value >> shift) & 63).unwrap_or_default();
            output.push(char::from(ALPHABET.get(index).copied().unwrap_or(b'A')));
        }
        match chunk.len() {
            1 => output.replace_range(output.len().saturating_sub(2).., "=="),
            2 => output.replace_range(output.len().saturating_sub(1).., "="),
            _ => {}
        }
    }
    output
}

/// Декодирует `authzid NUL authcid NUL password` и применяет authzid policy.
///
/// Пустой authzid означает авторизацию от имени authcid. Непустой authzid
/// обязан совпасть с authcid, иначе пароль одного account не даёт доступ к
/// другому account.
pub fn parse_plain(initial_response: &str) -> Result<PlainCredentials, &'static str> {
    let decoded = decode_base64(initial_response.as_bytes())?;
    if encode_base64(&decoded) != initial_response {
        return Err("noncanonical base64");
    }
    let mut fields = decoded.split(|byte| *byte == 0);
    let Some(authzid) = fields.next() else {
        return Err("missing authzid");
    };
    let Some(authcid) = fields.next() else {
        return Err("missing authcid");
    };
    let Some(password) = fields.next() else {
        return Err("missing password");
    };
    if fields.next().is_some() {
        return Err("unexpected NUL in SASL PLAIN field");
    }
    let account = std::str::from_utf8(authcid).map_err(|_| "invalid authcid")?;
    std::str::from_utf8(password).map_err(|_| "invalid password encoding")?;
    match (authzid.is_empty(), authzid == authcid, account.is_empty()) {
        (_, _, true) => Err("empty authcid"),
        (true, _, false) | (false, true, false) => Ok(PlainCredentials {
            account: account.to_owned(),
            password: password.to_vec(),
        }),
        (false, false, false) => Err("unauthorized authzid"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_empty_or_matching_authzid() {
        let parsed = parse_plain("AGJvYgB0ZXN0").expect("plain");
        assert_eq!(parsed.account, "bob");
        assert_eq!(parsed.password, b"test");
        assert!(parse_plain("YWxpY2UAYm9iAHRlc3Q").is_err());
    }

    #[test]
    fn rejects_missing_plain_fields() {
        assert!(parse_plain("AGJvYg").is_err());
        assert!(parse_plain("AGJvYgA").is_err());
    }

    #[test]
    fn rejects_noncanonical_base64_and_nul_inside_plain_fields() {
        assert!(parse_plain("AGJvYg==AAAA").is_err());
        assert!(parse_plain("AGJvYgB0ZXN0AA==").is_err());
        assert!(parse_plain("AGJvYgB0ZXN0 ").is_err());
    }

    #[test]
    fn client_encoding_round_trips_through_the_server_decoder() {
        let encoded = encode_plain("alice", b"secret");
        let decoded = parse_plain(&encoded).expect("round trip");
        assert_eq!(decoded.account, "alice");
        assert_eq!(decoded.password, b"secret");
    }
}
