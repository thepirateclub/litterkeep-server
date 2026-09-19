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
        let phc = Argon2::default()
            .hash_password(password)
            .map_err(invalid)?
            .to_string();
        Self::from_phc(phc)
    }
    pub fn verify(&self, password: &[u8]) -> bool {
        PasswordHash::new(&self.phc)
            .ok()
            .is_some_and(|hash| Argon2::default().verify_password(password, &hash).is_ok())
    }
    pub fn as_phc(&self) -> &str {
        &self.phc
    }
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
}
