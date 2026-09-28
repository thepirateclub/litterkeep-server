//! Известные ответы RFC 2202 для HMAC-SHA-1.
use super::authenticate;

/// Проверяет стандартный вектор с 20-байтовым ключом и длинный ключ.
#[test]
fn rfc_2202_known_answer_vectors_match() {
    let short_key = [0x0b; 20];
    assert_eq!(
        authenticate(&short_key, b"Hi There"),
        decode("b617318655057264e28bc0b6fb378c8ef146be00")
    );
    let long_key = [0xaa; 80];
    assert_eq!(
        authenticate(
            &long_key,
            b"Test Using Larger Than Block-Size Key - Hash Key First"
        ),
        decode("aa4ae5e15272d00e95705637ce8a3b55ed402112")
    );
}

/// Разбирает только фиксированные hex-строки тестовых RFC векторов.
fn decode(value: &str) -> [u8; 20] {
    let mut bytes = [0; 20];
    for (pair, slot) in value.as_bytes().chunks_exact(2).zip(bytes.iter_mut()) {
        let text = std::str::from_utf8(pair).expect("hex is ASCII");
        *slot = u8::from_str_radix(text, 16).expect("hex digit pair");
    }
    bytes
}
