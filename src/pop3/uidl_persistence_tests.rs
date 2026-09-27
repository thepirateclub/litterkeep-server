//! Проверяет долговечность POP3 UIDL поверх файлового Store.

use super::{Reply, Session, parse};
use crate::store::Store;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

/// Создаёт отдельный путь, чтобы параллельные тесты не делили maildrop.
fn temporary_store() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-pop3-uidl-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("часы должны быть доступны")
            .as_nanos()
    ))
}

/// Строит полный RFC 1939 multiline frame для ожидаемых UID.
fn expected_listing(entries: impl IntoIterator<Item = (u32, u64)>) -> Vec<u8> {
    let mut response = b"+OK unique-id listing follows\r\n".to_vec();
    for (number, uid) in entries {
        response.extend_from_slice(format!("{number} lk-{uid:016x}\r\n").as_bytes());
    }
    response.extend_from_slice(b".\r\n");
    response
}

/// Открывает POP3 TRANSACTION только через завершение typed authentication.
fn authenticated_session(messages: Vec<crate::store::Message>) -> Session {
    let mut session = Session::default();
    assert!(matches!(session.apply(parse("USER bob\r\n")), Reply::Ok(_)));
    assert!(matches!(session.authenticated(messages), Reply::Ok(_)));
    session
}

/// Читает UIDL multiline response без привязки к внутреннему UID formatter.
fn uidl_listing(session: &mut Session) -> Vec<u8> {
    match session.apply(parse("UIDL\r\n")) {
        Reply::Multiline(listing) => listing,
        _ => panic!("UIDL без аргумента обязан вернуть multiline listing"),
    }
}

#[test]
fn uidl_survives_sessions_restart_and_expunge_without_reuse() {
    let root = temporary_store();
    let first_uid;
    {
        let mut store = Store::open(&root).expect("первый Store");
        first_uid = store
            .append("bob", b"Subject: first\r\n\r\nbody")
            .expect("первое письмо");
        let snapshot = store.snapshot("bob").expect("первый snapshot");
        let mut session = authenticated_session(snapshot.messages);
        assert_eq!(
            uidl_listing(&mut session),
            expected_listing([(1, first_uid)])
        );
    }

    let second_uid;
    {
        let mut store = Store::open(&root).expect("повторно открытый Store");
        second_uid = store
            .append("bob", b"Subject: second\r\n\r\nbody")
            .expect("второе письмо");
        let snapshot = store.snapshot("bob").expect("восстановленный snapshot");
        let mut session = authenticated_session(snapshot.messages);
        assert_eq!(
            uidl_listing(&mut session),
            expected_listing([(1, first_uid), (2, second_uid)])
        );

        store
            .expunge("bob", &[first_uid])
            .expect("удаление первого UID");
        let third_uid = store
            .append("bob", b"Subject: third\r\n\r\nbody")
            .expect("третье письмо");
        assert_ne!(third_uid, first_uid);
        assert_ne!(third_uid, second_uid);

        let current = store.snapshot("bob").expect("текущий snapshot");
        let mut next_session = authenticated_session(current.messages);
        assert_eq!(
            uidl_listing(&mut next_session),
            expected_listing([(1, second_uid), (2, third_uid)])
        );
    }
    fs::remove_dir_all(root).expect("временный Store удалён");
}
