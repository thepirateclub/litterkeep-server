//! Проверки формирования nonce и полей server-first challenge.
use super::*;

#[test]
fn challenge_uses_client_nonce_prefix_and_verifier_parameters() {
    let client = client_first();
    let credential = credential();
    let challenge =
        ServerFirst::with_suffix(&client, &credential, "serverSuffix").expect("valid server nonce");
    let salt = crate::sasl::encode_base64(credential.salt());

    assert_eq!(challenge.nonce, "clientNonce-serverSuffix");
    assert_eq!(
        challenge.message,
        format!("r=clientNonce-serverSuffix,s={salt},i=4096")
    );
}

#[test]
fn secure_challenges_are_unique_and_keep_nonce_printable() {
    let client = client_first();
    let credential = credential();
    let first = ServerFirst::create(&client, &credential).expect("secure challenge");
    let second = ServerFirst::create(&client, &credential).expect("secure challenge");

    assert_ne!(first.nonce, second.nonce);
    assert!(first.nonce.starts_with(&client.nonce));
    assert!(
        first
            .nonce
            .bytes()
            .all(|octet| octet.is_ascii_graphic() && octet != b',')
    );
}

#[test]
fn rejects_empty_or_nonprintable_server_nonce_suffix() {
    let client = client_first();
    let credential = credential();
    for suffix in ["", "contains space", "comma,inside", "line\nbreak"] {
        assert!(ServerFirst::with_suffix(&client, &credential, suffix).is_err());
    }
}

/// Создаёт фиксированные client-first поля для изолированного challenge теста.
fn client_first() -> ClientFirst {
    ClientFirst {
        username: "alice".to_owned(),
        nonce: "clientNonce-".to_owned(),
        client_first_bare: "n=alice,r=clientNonce-".to_owned(),
    }
}

/// Получает SCRAM verifier без тестовых обходов закрытых ключей.
fn credential() -> ScramSha1Record {
    ScramSha1Record::create(b"alice password").expect("SCRAM verifier")
}
