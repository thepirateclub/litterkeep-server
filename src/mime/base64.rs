//! Однопроходные Base64-декодеры для MIME-тел и RFC 2047 encoded-word.

/// Декодирует Base64 с ошибкой на октетах вне алфавита.
pub fn decode_base64(input: &[u8]) -> Result<Vec<u8>, &'static str> {
    decode(input, InvalidOctetPolicy::Reject)
}

/// Декодирует MIME CTE Base64, игнорируя неалфавитные октеты по RFC 2045 §6.8.
pub fn decode_mime_base64(input: &[u8]) -> Result<Vec<u8>, &'static str> {
    decode(input, InvalidOctetPolicy::Ignore)
}

/// Профиль обработки октетов вне Base64-алфавита.
#[derive(Clone, Copy)]
enum InvalidOctetPolicy {
    Reject,
    Ignore,
}

/// Декодирует вход одним проходом, удерживая только текущий квартет.
fn decode(input: &[u8], policy: InvalidOctetPolicy) -> Result<Vec<u8>, &'static str> {
    let mut output = Vec::new();
    let mut quantum = Vec::with_capacity(4);
    let mut padding_seen = false;
    for byte in input.iter().copied() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        let Some(value) = symbol(byte) else {
            match policy {
                InvalidOctetPolicy::Reject => return Err("invalid base64 character"),
                InvalidOctetPolicy::Ignore => continue,
            }
        };
        if padding_seen {
            return Err("base64 data follows padding");
        }
        quantum.push(value);
        if quantum.len() == 4 {
            let packed: [u8; 4] = quantum
                .as_slice()
                .try_into()
                .map_err(|_| "invalid base64 quantum")?;
            padding_seen = decode_quantum(packed, &mut output)?;
            quantum.clear();
        }
    }
    match quantum.is_empty() {
        true => Ok(output),
        false => Err("invalid base64 width"),
    }
}

/// Преобразует один алфавитный символ или знак padding в шестибитное значение.
fn symbol(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        b'=' => Some(64),
        _ => None,
    }
}

/// Проверяет padding/нулевые pad bits и добавляет один Base64 quantum.
fn decode_quantum(quantum: [u8; 4], output: &mut Vec<u8>) -> Result<bool, &'static str> {
    let [first, second, third, fourth] = quantum;
    match (first == 64, second == 64, third == 64, fourth == 64) {
        (true, _, _, _) | (_, true, _, _) => Err("invalid base64 padding"),
        (false, false, true, true) if second & 0b1111 == 0 => {
            output.push((first << 2) | (second >> 4));
            Ok(true)
        }
        (false, false, false, true) if third & 0b11 == 0 => {
            output.push((first << 2) | (second >> 4));
            output.push((second << 4) | (third >> 2));
            Ok(true)
        }
        (false, false, false, false) => {
            output.push((first << 2) | (second >> 4));
            output.push((second << 4) | (third >> 2));
            output.push((third << 6) | fourth);
            Ok(false)
        }
        _ => Err("invalid base64 padding"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_decoder_rejects_non_alphabet_octets() {
        assert!(decode_base64(b"Y!WJj").is_err());
        assert_eq!(decode_base64(b"YWI=\r\n").expect("strict Base64"), b"ab");
    }

    #[test]
    fn mime_decoder_ignores_arbitrary_non_alphabet_octets() {
        assert_eq!(
            decode_mime_base64(b"Y!WJ#j\xff").expect("MIME Base64"),
            b"abc"
        );
    }

    #[test]
    fn both_decoders_reject_invalid_width_padding_and_data_after_padding() {
        for input in [b"xx=".as_slice(), b"YQ==AAAA", b"YR==", b"YWJ=", b"YQ==="] {
            assert!(decode_base64(input).is_err(), "accepted {input:?}");
            assert!(decode_mime_base64(input).is_err(), "accepted {input:?}");
        }
    }
}
