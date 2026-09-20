//! Повторяемая первичная инициализация локальных acceptance-аккаунтов.

use crate::accounts::Accounts;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const SECRET_FILE: &str = "test-accounts.tsv";

/// Фиксированный набор локальных владельцев из acceptance-контракта.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Account {
    Alice,
    Bob,
    Qa,
    Pop,
}

impl Account {
    fn all() -> [Self; 4] {
        [Self::Alice, Self::Bob, Self::Qa, Self::Pop]
    }

    fn name(self) -> &'static str {
        match self {
            Self::Alice => "alice",
            Self::Bob => "bob",
            Self::Qa => "qa",
            Self::Pop => "pop",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "alice" => Some(Self::Alice),
            "bob" => Some(Self::Bob),
            "qa" => Some(Self::Qa),
            "pop" => Some(Self::Pop),
            _ => None,
        }
    }
}

/// Итог без раскрытия сгенерированных секретов в stdout или журнал.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapReport {
    pub credentials_path: PathBuf,
    pub created_accounts: usize,
    pub existing_accounts: usize,
}

/// Сначала долговечно фиксирует секреты, затем идемпотентно создаёт PHC records.
pub fn initialize(root: impl AsRef<Path>) -> io::Result<BootstrapReport> {
    let root = root.as_ref();
    fs::create_dir_all(root)?;
    let credentials_path = root.join(SECRET_FILE);
    let credentials = match fs::read_to_string(&credentials_path) {
        Ok(value) => parse_credentials(&value)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let generated = generate_credentials()?;
            publish_credentials(&credentials_path, &generated)?;
            generated
        }
        Err(error) => return Err(error),
    };
    let accounts = Accounts::open(root)?;
    let mut created_accounts = 0_usize;
    let mut existing_accounts = 0_usize;
    for (account, password) in &credentials {
        match accounts.create(account.name(), password.as_bytes()) {
            Ok(()) => created_accounts = increment(created_accounts)?,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                if !accounts.authenticate(account.name(), password.as_bytes()) {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "existing account does not match saved bootstrap credential",
                    ));
                }
                existing_accounts = increment(existing_accounts)?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(BootstrapReport {
        credentials_path,
        created_accounts,
        existing_accounts,
    })
}

fn generate_credentials() -> io::Result<Vec<(Account, String)>> {
    let random = rustls::crypto::ring::default_provider().secure_random;
    Account::all()
        .into_iter()
        .map(|account| {
            let mut bytes = [0_u8; 24];
            random
                .fill(&mut bytes)
                .map_err(|_| io::Error::other("secure password generation failed"))?;
            Ok((account, encode_hex(&bytes)))
        })
        .collect()
}

fn publish_credentials(path: &Path, credentials: &[(Account, String)]) -> io::Result<()> {
    let mut file = secret_file(path)?;
    for (account, password) in credentials {
        writeln!(file, "{}\t{password}", account.name())?;
    }
    file.sync_all()?;
    sync_directory(path.parent().unwrap_or_else(|| Path::new(".")))
}

#[cfg(target_os = "linux")]
fn secret_file(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(target_os = "linux"))]
fn secret_file(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

fn parse_credentials(value: &str) -> io::Result<Vec<(Account, String)>> {
    let parsed = value
        .lines()
        .map(|line| {
            let mut fields = line.split('\t');
            let account = fields.next().and_then(Account::parse);
            let password = fields.next();
            match (account, password, fields.next()) {
                (Some(account), Some(password), None) if !password.is_empty() => {
                    Ok((account, password.to_owned()))
                }
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid bootstrap credential file",
                )),
            }
        })
        .collect::<io::Result<Vec<_>>>()?;
    let exact = Account::all().into_iter().all(|required| {
        parsed
            .iter()
            .filter(|(account, _)| *account == required)
            .count()
            == 1
    });
    match (parsed.len(), exact) {
        (4, true) => Ok(parsed),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bootstrap credential set is incomplete",
        )),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn increment(value: usize) -> io::Result<usize> {
    value
        .checked_add(1)
        .ok_or_else(|| io::Error::other("bootstrap account count overflow"))
}

#[cfg(target_os = "linux")]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(target_os = "linux"))]
fn sync_directory(path: &Path) -> io::Result<()> {
    match path.try_exists()? {
        true => Ok(()),
        false => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "data directory missing",
        )),
    }
}

#[cfg(test)]
#[path = "bootstrap_tests.rs"]
mod tests;
