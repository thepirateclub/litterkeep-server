//! PBKDF2-HMAC-SHA-1 known-answer vectors из RFC 6070.
use super::derive;

/// Проверяет тестовые итерации, рабочую SCRAM-нагрузку и нулевой отказ.
#[test]
fn rfc_6070_known_answer_vectors_match() {
    assert_eq!(
        derive(b"password", b"salt", 1),
        Some(decode("0c60c80f961f0e71f3a9b524af6012062fe037a6"))
    );
    assert_eq!(
        derive(b"password", b"salt", 2),
        Some(decode("ea6c014dc72d6f8ccd1ed92ace1d41f0d8de8957"))
    );
    assert_eq!(
        derive(b"password", b"salt", 4096),
        Some(decode("4b007901b765489abead49d926f721d065a429c1"))
    );
    assert_eq!(derive(b"password", b"salt", 0), None);
}

/// Разбирает только фиксированные hex-строки тестовых RFC векторов.
fn decode(value: &str) -> [u8; 20] {
    let mut bytes = [0; 20];
    for (pair, slot) in value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .zip(bytes.iter_mut())
    {
        let text = std::str::from_utf8(pair).expect("hex is ASCII");
        *slot = u8::from_str_radix(text, 16).expect("hex digit pair");
    }
    bytes
}
