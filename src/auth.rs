//! Argon2id password credentials. Hash text is the portable PHC record.
use argon2::{
    Algorithm, Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use std::io;

#[derive(Clone, Debug)]
pub struct Credential {
    phc: String,
}

impl Credential {
    pub fn from_phc(phc: String) -> io::Result<Self> {
        let parsed = PasswordHash::new(&phc).map_err(invalid)?;
        match parsed.algorithm.as_str().parse::<Algorithm>() {
            Ok(Algorithm::Argon2id) => {}
            Ok(_) | Err(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "credential is not Argon2id",
                ));
            }
        }
        Ok(Self { phc })
    }
    pub fn create(password: &[u8]) -> io::Result<Self> {
        let password = prepare_stored_password(password)?;
        let phc = Argon2::default()
            .hash_password(&password)
            .map_err(invalid)?
            .to_string();
        Self::from_phc(phc)
    }
    pub fn verify(&self, password: &[u8]) -> bool {
        let normalized = match prepare_query_password(password) {
            Ok(password) => password,
            Err(_) => return false,
        };
        self.verify_bytes(&normalized)
    }
    /// Проверяет пароль без нормализации для явно выбранного старого PHC-файла.
    pub(crate) fn verify_legacy_raw(&self, password: &[u8]) -> bool {
        self.verify_bytes(password)
    }
    fn verify_bytes(&self, password: &[u8]) -> bool {
        PasswordHash::new(&self.phc)
            .ok()
            .is_some_and(|hash| Argon2::default().verify_password(password, &hash).is_ok())
    }
    pub fn as_phc(&self) -> &str {
        &self.phc
    }
}

/// Преобразует новый сохранённый секрет через строгий SASLprep stored policy.
fn prepare_stored_password(password: &[u8]) -> io::Result<Vec<u8>> {
    let text = std::str::from_utf8(password).map_err(invalid_password)?;
    let prepared =
        crate::sasl::saslprep::prepare(text, crate::sasl::saslprep::SaslPrepContext::StoredValue)
            .map_err(invalid_password)?;
    Ok(prepared.into_bytes())
}

/// Нормализует попытку входа и допускает query-разрешённые unassigned scalars.
fn prepare_query_password(password: &[u8]) -> io::Result<Vec<u8>> {
    let text = std::str::from_utf8(password).map_err(invalid_password)?;
    let prepared =
        crate::sasl::saslprep::prepare(text, crate::sasl::saslprep::SaslPrepContext::Query)
            .map_err(invalid_password)?;
    Ok(prepared.into_bytes())
}

/// Представляет некорректный UTF-8 или пароль, запрещённый SASLprep.
fn invalid_password(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error.to_string())
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn argon2id_rejects_wrong_password() {
        let c = Credential::create(b"test-password").unwrap();
        assert!(c.verify(b"test-password"));
        assert!(!c.verify(b"wrong"));
        assert!(Credential::from_phc(c.as_phc().into()).is_ok());
    }

    /// Сохраняемый Argon2 хэш зависит от SASLprep-представления пароля.
    #[test]
    fn argon2_credentials_accept_saslprep_equivalent_queries() {
        let credential = Credential::create("\u{00AA}".as_bytes()).unwrap();
        assert!(credential.verify(b"a"));
        assert!(credential.verify("\u{00AA}".as_bytes()));
        assert!(!credential.verify(b"A"));
    }

    /// Новая запись не может хранить prohibited или unassigned пароль.
    #[test]
    fn argon2_credential_creation_rejects_invalid_saslprep_input() {
        assert_eq!(
            Credential::create(&[0xFF]).err().map(|error| error.kind()),
            Some(io::ErrorKind::InvalidInput)
        );
        assert_eq!(
            Credential::create("no\u{0007}".as_bytes())
                .err()
                .map(|error| error.kind()),
            Some(io::ErrorKind::InvalidInput)
        );
    }
}
