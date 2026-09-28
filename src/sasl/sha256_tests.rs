//! Проверяет собственный SHA-256 по опубликованным контрольным векторам.

use super::digest;

/// Форматирует digest в lowercase hex только для сравнения в тестах.
fn hexadecimal(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Сверяет короткие и многоблочные сообщения с RFC 6234 §8.4.
#[test]
fn rfc_6234_sha256_known_answers_match() {
    let cases: [(&[u8], &str); 3] = [
        (
            b"",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
    ];
    for (message, expected) in cases {
        assert_eq!(hexadecimal(&digest(message)), expected);
    }
}

/// Пересекает границы padding и блока, где ошибаются неверные реализации.
#[test]
fn padding_boundaries_match_reference_digests() {
    assert_eq!(
        hexadecimal(&digest(&[b'a'; 55])),
        "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"
    );
    assert_eq!(
        hexadecimal(&digest(&[b'a'; 56])),
        "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"
    );
}

/// Миллион повторов подтверждает multi-block обработку большого сообщения.
#[test]
fn million_octet_reference_vector_matches() {
    assert_eq!(
        hexadecimal(&digest(&vec![b'a'; 1_000_000])),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
}
