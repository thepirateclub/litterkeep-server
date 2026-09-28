//! File-backed account credentials. Password hashing stays outside the reactor.
use crate::auth::Credential;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "accounts/identity.rs"]
mod identity;
#[path = "accounts/scram_store.rs"]
mod scram_store;

static NEXT_MIGRATION_ID: AtomicU64 = AtomicU64::new(1);

pub struct Accounts {
    root: PathBuf,
}
impl Accounts {
    pub fn open(data: impl AsRef<Path>) -> io::Result<Self> {
        let root = data.as_ref().join("accounts");
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }
    pub fn create(&self, name: &str, password: &[u8]) -> io::Result<()> {
        let name = identity::stored_name(name)?;
        if self.has_identity_record(&name)? {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "account identity already exists or conflicts with legacy key",
            ));
        }
        let credential = Credential::create(password)?;
        let path = crate::store::file_key::prepare(
            &self.root,
            &name,
            crate::store::file_key::FileKind::SaslPrepCredentials,
        )?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(credential.as_phc().as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        let directory = path.parent().unwrap_or(&self.root);
        crate::store::file_key::sync_key_directory(&self.root, directory)?;
        match scram_store::create(&self.root, &name, password) {
            Ok(()) => Ok(()),
            Err(error) => {
                let credential_cleanup = fs::remove_file(path);
                let scram_cleanup = scram_store::remove(&self.root, &name);
                match (credential_cleanup, scram_cleanup) {
                    (Ok(()), Ok(())) => Err(error),
                    (credential_result, scram_result) => Err(io::Error::new(
                        error.kind(),
                        format!(
                            "{error}; account rollback credential={credential_result:?}, SCRAM={scram_result:?}"
                        ),
                    )),
                }
            }
        }
    }

    pub fn authenticate(&self, name: &str, password: &[u8]) -> io::Result<bool> {
        let name = identity::query_name(name)?;
        let normalized_path = self.normalized_path(&name);
        match fs::read_to_string(normalized_path) {
            Ok(phc) => match verify_credential(&phc, password, CredentialFormat::SaslPrep)? {
                false => Ok(false),
                true => {
                    scram_store::ensure(&self.root, &name, password)?;
                    Ok(true)
                }
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.authenticate_legacy(&name, password)
            }
            Err(error) => Err(error),
        }
    }

    /// Загружает SCRAM verifier для protocol owner без раскрытия пароля.
    pub fn scram_verifier(
        &self,
        name: &str,
    ) -> io::Result<Option<crate::sasl::scram_sha1_record::ScramSha1Record>> {
        let name = identity::query_name(name)?;
        scram_store::load(&self.root, &name)
    }
    /// Сообщает, существует ли допустимый локальный получатель.
    ///
    /// Эта проверка намеренно не раскрывает credential и нужна writer-у до
    /// публикации SMTP-сообщения в mailbox получателя.
    pub fn exists(&self, name: &str) -> bool {
        let Ok(name) = identity::query_name(name) else {
            return false;
        };
        match (
            self.path(&name).is_file(),
            self.normalized_path(&name).is_file(),
        ) {
            (true, _) | (_, true) => true,
            (false, false) => false,
        }
    }
    fn path(&self, name: &str) -> PathBuf {
        crate::store::file_key::path(
            &self.root,
            name,
            crate::store::file_key::FileKind::Credentials,
        )
    }

    /// Возвращает отдельный typed path для SASLprep-нормализованных Argon2 записей.
    fn normalized_path(&self, name: &str) -> PathBuf {
        crate::store::file_key::path(
            &self.root,
            name,
            crate::store::file_key::FileKind::SaslPrepCredentials,
        )
    }

    /// Оставляет старые PHC читаемыми, не выдавая их за нормализованные записи.
    fn authenticate_legacy(&self, name: &str, password: &[u8]) -> io::Result<bool> {
        match fs::read_to_string(self.path(name)) {
            Ok(phc) => match verify_credential(&phc, password, CredentialFormat::LegacyRaw)? {
                false => Ok(false),
                true => {
                    self.migrate_legacy(name, password)?;
                    Ok(true)
                }
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Переписывает проверенную legacy-запись в новом формате до успешного входа.
    fn migrate_legacy(&self, name: &str, password: &[u8]) -> io::Result<()> {
        let credential = Credential::create(password)?;
        scram_store::ensure(&self.root, name, password)?;
        let path = crate::store::file_key::prepare(
            &self.root,
            name,
            crate::store::file_key::FileKind::SaslPrepCredentials,
        )?;
        let temporary = migration_temporary_path(&path)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let preparation = (|| {
            file.write_all(credential.as_phc().as_bytes())?;
            file.write_all(b"\n")?;
            file.sync_all()
        })();
        drop(file);
        if let Err(error) = preparation {
            return match fs::remove_file(&temporary) {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(io::Error::new(
                    error.kind(),
                    format!("{error}; migration temporary cleanup failed: {cleanup_error}"),
                )),
            };
        }
        let publication = fs::hard_link(&temporary, &path);
        let cleanup = fs::remove_file(&temporary);
        match (publication, cleanup) {
            (Ok(()), Ok(())) => {
                let directory = path.parent().unwrap_or(&self.root);
                crate::store::file_key::sync_key_directory(&self.root, directory)
            }
            (Err(error), Ok(())) if error.kind() == io::ErrorKind::AlreadyExists => {
                self.verify_concurrent_migration(&path, password)
            }
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Err(error)) => Err(error),
            (Err(error), Err(cleanup_error)) => Err(io::Error::new(
                error.kind(),
                format!("{error}; migration temporary cleanup failed: {cleanup_error}"),
            )),
        }
    }

    /// Подтверждает результат конкурирующей миграции, не принимая сменённый пароль.
    fn verify_concurrent_migration(&self, path: &Path, password: &[u8]) -> io::Result<()> {
        let phc = fs::read_to_string(path)?;
        match verify_credential(&phc, password, CredentialFormat::SaslPrep)? {
            true => Ok(()),
            false => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "credential changed during legacy migration",
            )),
        }
    }

    /// Запрещает новую учётку при конфликте с любым старым эквивалентным ключом.
    fn has_identity_record(&self, canonical: &str) -> io::Result<bool> {
        let kinds = [
            crate::store::file_key::FileKind::Credentials,
            crate::store::file_key::FileKind::SaslPrepCredentials,
            crate::store::file_key::FileKind::ScramCredentials,
        ];
        for kind in kinds {
            let entries = crate::store::file_key::entries(&self.root, kind)?;
            for (stored, _) in entries {
                match identity::query_name(&stored) {
                    Ok(prepared) if prepared == canonical => return Ok(true),
                    Ok(_) | Err(_) => {}
                }
            }
        }
        Ok(false)
    }
}

/// Канонизирует протокольное имя аккаунта по query-профилю SASLprep.
pub(crate) fn canonical_query_name(name: &str) -> Option<String> {
    identity::query_name(name).ok()
}

/// Формирует уникальный временный путь в том же каталоге для атомарной публикации.
fn migration_temporary_path(path: &Path) -> io::Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing credential parent"))?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let identifier = NEXT_MIGRATION_ID.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(
        ".phc2-migration-{}-{timestamp}-{identifier}.tmp",
        std::process::id()
    )))
}

/// Выбирает password semantics по типизированному storage format.
#[derive(Clone, Copy)]
enum CredentialFormat {
    LegacyRaw,
    SaslPrep,
}

/// Выбирает password semantics по типизированному storage format.
fn verify_credential(phc: &str, password: &[u8], format: CredentialFormat) -> io::Result<bool> {
    let credential = match Credential::from_phc(phc.trim().to_owned()) {
        Ok(credential) => credential,
        Err(_) => return Ok(false),
    };
    let accepted = match format {
        CredentialFormat::LegacyRaw => match credential.verify(password) {
            true => true,
            false => credential.verify_legacy_raw(password),
        },
        CredentialFormat::SaslPrep => credential.verify(password),
    };
    Ok(accepted)
}
#[cfg(test)]
#[path = "accounts_tests.rs"]
mod tests;
