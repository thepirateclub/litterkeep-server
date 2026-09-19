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

/// Декодирует `authzid NUL authcid NUL password` и применяет authzid policy.
///
/// Пустой authzid означает авторизацию от имени authcid. Непустой authzid
/// обязан совпасть с authcid, иначе пароль одного account не даёт доступ к
/// другому account.
pub fn parse_plain(initial_response: &str) -> Result<PlainCredentials, &'static str> {
    let decoded = decode_base64(initial_response.as_bytes())?;
    let mut fields = decoded.splitn(3, |byte| *byte == 0);
    let Some(authzid) = fields.next() else { return Err("missing authzid"); };
    let Some(authcid) = fields.next() else { return Err("missing authcid"); };
    let Some(password) = fields.next() else { return Err("missing password"); };
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
}
