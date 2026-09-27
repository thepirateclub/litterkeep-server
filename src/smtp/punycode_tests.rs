//! Проверки RFC-векторов и защитных границ Punycode codec.

use super::{Error, decode, encode};

/// Векторы RFC 3492 §7.1 проверяют кодирование разных Unicode-письменностей.
#[test]
fn matches_rfc_3492_multilingual_vectors() {
    for (unicode, punycode) in [
        ("他们为什么不说中文", "ihqwcrb4cv8a8dqg056pqjye"),
        ("3年B組金八先生", "3B-ww4c5e180e575a65lsy2b"),
    ] {
        assert_eq!(encode(unicode), Ok(punycode.to_owned()));
        assert_eq!(decode(punycode), Ok(unicode.to_owned()));
    }
}

/// ASCII-коды сохраняются, а смешанная строка проходит обратное преобразование.
#[test]
fn preserves_basic_code_points_and_round_trips() {
    for value in ["bücher", "mañana", "A-例え-9"] {
        let encoded = encode(value).expect("valid Unicode scalar input");
        assert_eq!(decode(&encoded), Ok(value.to_owned()));
    }
    assert_eq!(encode(""), Ok(String::new()));
    assert_eq!(encode("ascii"), Ok("ascii-".to_owned()));
    assert_eq!(encode("bücher"), Ok("bcher-kva".to_owned()));
}

/// Декодер отвергает неверные цифры, усечённые числа и surrogate code points.
#[test]
fn rejects_malformed_or_overflowing_sequences() {
    for value in ["a*", "z-zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"] {
        assert!(matches!(
            decode(value),
            Err(Error::InvalidInput | Error::Overflow)
        ));
    }
    assert_eq!(decode("abc-"), Ok("abc".to_owned()));
    assert_eq!(decode("é"), Err(Error::InvalidInput));
    assert_eq!(encode(&"a".repeat(1025)), Err(Error::InputTooLong));
}
