//! Независимые официальные SHA-1 известные ответы RFC 3174.
use super::digest;

/// Представляет digest только в тесте, чтобы сравнить его с RFC hex vector.
fn hexadecimal(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Проверяет пустой ввод, короткий ввод и сообщение длиной в несколько блоков.
#[test]
fn rfc_3174_known_answer_vectors_match() {
    let cases: [(&[u8], &str); 3] = [
        (b"", "da39a3ee5e6b4b0d3255bfef95601890afd80709"),
        (b"abc", "a9993e364706816aba3e25717850c26c9cd0d89d"),
        (
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1",
        ),
    ];
    for (message, expected) in cases {
        assert_eq!(hexadecimal(&digest(message)), expected);
    }
}
