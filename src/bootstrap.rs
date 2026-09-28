//! Повторяемая первичная инициализация локальных acceptance-аккаунтов.

use crate::accounts::Accounts;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const SECRET_FILE: &str = "test-accounts.tsv";
const SMTPUTF8_SECRET_FILE: &str = "test-smtputf8-account.tsv";

/// Фиксированный набор локальных владельцев из acceptance-контракта.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Account {
    Alice,
    Bob,
    Qa,
    Pop,
    UnicodeRecipient,
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
            Self::UnicodeRecipient => "пользователь",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "alice" => Some(Self::Alice),
            "bob" => Some(Self::Bob),
            "qa" => Some(Self::Qa),
            "pop" => Some(Self::Pop),
            "пользователь" => Some(Self::UnicodeRecipient),
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

/// Итог явного opt-in создания Unicode-получателя для SMTPUTF8 acceptance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmtpUtf8TestAccountReport {
    pub credentials_path: PathBuf,
    pub created: bool,
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
        match create_or_validate(&accounts, *account, password.as_bytes())? {
            true => created_accounts = increment(created_accounts)?,
            false => existing_accounts = increment(existing_accounts)?,
        }
    }
    Ok(BootstrapReport {
        credentials_path,
        created_accounts,
        existing_accounts,
    })
}

/// Инициализирует стандартный fixture и отдельно добавляет Unicode account.
///
/// Обычный `init` сохраняет исторический набор аккаунтов и credentials без
/// изменения. Этот явный opt-in хранит пароль в отдельном файле с тем же mode.
pub fn initialize_smtp_utf8_test(root: impl AsRef<Path>) -> io::Result<SmtpUtf8TestAccountReport> {
    let root = root.as_ref();
    initialize(root)?;
    let credentials_path = root.join(SMTPUTF8_SECRET_FILE);
    let password = read_or_create_smtp_utf8_password(&credentials_path)?;
    let accounts = Accounts::open(root)?;
    let created = create_or_validate(&accounts, Account::UnicodeRecipient, password.as_bytes())?;
    Ok(SmtpUtf8TestAccountReport {
        credentials_path,
        created,
    })
}

/// Возвращает прежний секрет либо публикует новый до создания account record.
fn read_or_create_smtp_utf8_password(path: &Path) -> io::Result<String> {
    match fs::read_to_string(path) {
        Ok(contents) => parse_smtp_utf8_password(&contents),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let password = generate_password()?;
            match publish_smtp_utf8_password(path, &password) {
                Ok(()) => Ok(password),
                Err(publish_error) if publish_error.kind() == io::ErrorKind::AlreadyExists => {
                    fs::read_to_string(path)
                        .and_then(|contents| parse_smtp_utf8_password(&contents))
                }
                Err(publish_error) => Err(publish_error),
            }
        }
        Err(error) => Err(error),
    }
}

/// Проверяет sidecar как единственную запись только для Unicode test account.
fn parse_smtp_utf8_password(contents: &str) -> io::Result<String> {
    let mut lines = contents.lines();
    let Some(line) = lines.next() else {
        return Err(invalid_credentials());
    };
    let mut fields = line.split('\t');
    let account = fields.next().and_then(Account::parse);
    let password = fields.next();
    match (account, password, fields.next(), lines.next()) {
        (Some(Account::UnicodeRecipient), Some(password), None, None) if !password.is_empty() => {
            Ok(password.to_owned())
        }
        _ => Err(invalid_credentials()),
    }
}

/// Публикует sidecar с теми же ограничениями доступа и fsync, что и основной.
fn publish_smtp_utf8_password(path: &Path, password: &str) -> io::Result<()> {
    let mut file = secret_file(path)?;
    writeln!(file, "{}\t{password}", Account::UnicodeRecipient.name())?;
    file.sync_all()?;
    sync_directory(path.parent().unwrap_or_else(|| Path::new(".")))
}

/// Создаёт credential либо подтверждает, что сохранённый пароль ей соответствует.
fn create_or_validate(accounts: &Accounts, account: Account, password: &[u8]) -> io::Result<bool> {
    match accounts.create(account.name(), password) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            match accounts.authenticate(account.name(), password)? {
                true => Ok(false),
                false => Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "existing account does not match saved bootstrap credential",
                )),
            }
        }
        Err(error) => Err(error),
    }
}

fn generate_credentials() -> io::Result<Vec<(Account, String)>> {
    Account::all()
        .into_iter()
        .map(|account| generate_password().map(|password| (account, password)))
        .collect()
}

/// Генерирует POP3-совместимый тестовый пароль длиной ровно 40 символов.
fn generate_password() -> io::Result<String> {
    let random = rustls::crypto::ring::default_provider().secure_random;
    let mut bytes = [0_u8; 20];
    random
        .fill(&mut bytes)
        .map_err(|_| io::Error::other("secure password generation failed"))?;
    Ok(encode_hex(&bytes))
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

/// Создаёт единообразную ошибку без включения возможного секрета в сообщение.
fn invalid_credentials() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid SMTPUTF8 test credentials",
    )
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
