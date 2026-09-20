//! Разбор initial response механизма SASL PLAIN без готового SASL framework.
//!
//! Модуль не проверяет пароль. Он формирует типизированные credentials, которые
//! затем передаются единственному владельцу account-хранилища.
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
    let mut fields = decoded.splitn(3, |byte| *byte == 0);
    let Some(authzid) = fields.next() else {
        return Err("missing authzid");
    };
    let Some(authcid) = fields.next() else {
        return Err("missing authcid");
    };
    let Some(password) = fields.next() else {
        return Err("missing password");
    };
    let account = std::str::from_utf8(authcid).map_err(|_| "invalid authcid")?;
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
    fn client_encoding_round_trips_through_the_server_decoder() {
        let encoded = encode_plain("alice", b"secret");
        let decoded = parse_plain(&encoded).expect("round trip");
        assert_eq!(decoded.account, "alice");
        assert_eq!(decoded.password, b"secret");
    }
}
