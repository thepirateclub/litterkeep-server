//! Проверки передачи SCRAM verifier через единственного владельца Accounts.

use super::{
    Accounts, Request, Response, StorageWorker,
    sasl_lookup::{
        ScramSha256VerifierRequest, ScramSha256VerifierResponse, ScramVerifierRequest,
        ScramVerifierResponse,
    },
};

/// Убеждается, что reactor получает запись только после bounded worker-запроса.
#[test]
fn verifier_lookup_stays_inside_the_accounts_owner() {
    let root = test_root();
    let accounts = Accounts::open(&root).expect("учётные записи");
    accounts.create("bob", b"test-password").expect("аккаунт");
    let worker = StorageWorker::start(&root, 1).expect("worker");
    worker
        .submit(Request::ScramVerifier(ScramVerifierRequest {
            request_id: 12,
            account: "bob".into(),
        }))
        .expect("запрос принят");
    let response = receive(&worker);
    assert!(matches!(
        response,
        Response::ScramVerifier(ScramVerifierResponse {
            request_id: 12,
            result: Ok(Some(_)),
        })
    ));
    worker.shutdown();
    std::fs::remove_dir_all(root).expect("удаление временной fixture");
}

/// SHA-256 record выдаётся только через ограниченный Accounts owner channel.
#[test]
fn sha256_verifier_lookup_stays_inside_the_accounts_owner() {
    let root = test_root();
    let accounts = Accounts::open(&root).expect("учётные записи");
    accounts.create("bob", b"test-password").expect("аккаунт");
    let worker = StorageWorker::start(&root, 1).expect("worker");
    worker
        .submit(Request::ScramSha256Verifier(ScramSha256VerifierRequest {
            request_id: 13,
            account: "bob".into(),
        }))
        .expect("запрос принят");
    let response = receive(&worker);
    assert!(matches!(
        response,
        Response::ScramSha256Verifier(ScramSha256VerifierResponse {
            request_id: 13,
            result: Ok(Some(_)),
        })
    ));
    worker.shutdown();
    std::fs::remove_dir_all(root).expect("удаление временной fixture");
}

/// Ожидает завершение одного запроса, не открывая Store в тестовом потоке.
fn receive(worker: &StorageWorker) -> Response {
    // Конечный бюджет обязателен: без него зависший воркер держит CI-джоб до
    // его общий таймаут. Пауза вместо yield_now оставляет CPU потоку воркера.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        match worker.try_response() {
            Ok(response) => return response,
            Err(std::sync::mpsc::TryRecvError::Empty) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                panic!("worker не ответил до истечения бюджета ожидания")
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                panic!("worker завершился до completion")
            }
        }
    }
}

/// Создаёт уникальный каталог fixture, чтобы параллельные тесты не пересеклись.
fn test_root() -> std::path::PathBuf {
    let identifier = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("часы")
        .as_nanos();
    std::env::temp_dir().join(format!("litterkeep-scram-worker-{identifier}"))
}
