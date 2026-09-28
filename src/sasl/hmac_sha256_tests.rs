//! Сверяет HMAC-SHA-256 с тестовыми векторами RFC 4231.

use super::authenticate;

/// Представляет байты как lowercase hex только в проверочных утверждениях.
fn hexadecimal(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Проверяет короткие и превышающие block size ключи из RFC 4231.
#[test]
fn rfc_4231_hmac_sha256_known_answers_match() {
    let cases = [
        (
            vec![0x0b; 20],
            b"Hi There".as_slice(),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7",
        ),
        (
            b"Jefe".to_vec(),
            b"what do ya want for nothing?".as_slice(),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843",
        ),
        (
            vec![0xaa; 131],
            b"Test Using Larger Than Block-Size Key - Hash Key First".as_slice(),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54",
        ),
    ];
    for (key, message, expected) in cases {
        assert_eq!(hexadecimal(&authenticate(&key, message)), expected);
    }
}
