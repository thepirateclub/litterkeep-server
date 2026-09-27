//! Ограниченное кодирование RFC 2045 для UTF-8 текста в письмах автоответа.

/// Кодирует octets за O(n) и оставляет soft line length не более 76.
pub(crate) fn encode(input: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(input.len());
    let mut line_length = 0_usize;
    let mut cursor = 0_usize;
    while let Some(byte) = input.get(cursor).copied() {
        match byte {
            b'\r' if input.get(cursor.saturating_add(1)) == Some(&b'\n') => {
                output.extend_from_slice(b"\r\n");
                line_length = 0;
                cursor = cursor.saturating_add(2);
            }
            b'\r' | b'\n' => {
                let token = escaped_octet(byte);
                append_token(&mut output, &mut line_length, &token);
                cursor = cursor.saturating_add(1);
            }
            b'!'..=b'<' | b'>'..=b'~' => {
                append_token(&mut output, &mut line_length, &[byte]);
                cursor = cursor.saturating_add(1);
            }
            _ => {
                let token = escaped_octet(byte);
                append_token(&mut output, &mut line_length, &token);
                cursor = cursor.saturating_add(1);
            }
        }
    }
    output
}

/// Строит нормативный `=HH` token на стеке без временной heap-строки.
fn escaped_octet(octet: u8) -> [u8; 3] {
    [b'=', hex_digit(octet >> 4), hex_digit(octet & 0x0f)]
}

/// Преобразует одну половину октета в uppercase hexadecimal ASCII.
fn hex_digit(value: u8) -> u8 {
    let nibble = value & 0x0f;
    match nibble <= 9 {
        true => b'0' + nibble,
        false => b'A' + (nibble - 10),
    }
}

fn append_token(output: &mut Vec<u8>, line_length: &mut usize, token: &[u8]) {
    if line_length.saturating_add(token.len()) > 75 {
        output.extend_from_slice(b"=\r\n");
        *line_length = 0;
    }
    output.extend_from_slice(token);
    *line_length = line_length.saturating_add(token.len());
}

#[cfg(test)]
mod tests {
    use super::encode;

    /// Сверяет каждый входной октет с RFC literal range либо независимым `=HH`.
    #[test]
    fn encodes_the_complete_octet_range_without_changing_content() {
        for octet in 0_u8..=u8::MAX {
            let encoded = encode(&[octet]);
            let expected = match octet {
                33..=60 | 62..=126 => vec![octet],
                _ => format!("={octet:02X}").into_bytes(),
            };
            assert_eq!(encoded, expected, "encoded octet {octet:02X}");
            assert_eq!(
                crate::mime::quoted_printable::decode(&encoded),
                Ok(vec![octet]),
                "decoded encoded octet {octet:02X}"
            );
        }
    }

    /// Канонические CRLF остаются hard breaks, одиночные CR/LF кодируются.
    #[test]
    fn preserves_only_complete_crlf_pairs_as_text_line_breaks() {
        assert_eq!(encode(b"a\r\nb"), b"a\r\nb");
        assert_eq!(encode(b"\r"), b"=0D");
        assert_eq!(encode(b"\n"), b"=0A");
    }

    /// Проверяет folding и byte-exact round-trip на смешанном полном диапазоне.
    #[test]
    fn folds_the_complete_octet_range_without_losing_bytes() {
        let source = (0_u8..=u8::MAX).collect::<Vec<_>>();
        let encoded = encode(&source);
        assert_eq!(crate::mime::quoted_printable::decode(&encoded), Ok(source));
        for line in encoded.split(|byte| *byte == b'\n') {
            let content = line.strip_suffix(b"\r").unwrap_or(line);
            assert!(content.len() <= 76);
        }
    }

    #[test]
    fn encoded_utf8_round_trips_and_folds_within_wire_limit() {
        let source = "Письмо 📨 ".repeat(20);
        let encoded = encode(source.as_bytes());
        assert_eq!(
            crate::mime::quoted_printable::decode(&encoded).expect("decode"),
            source.as_bytes()
        );
        for line in encoded.split(|byte| *byte == b'\n') {
            let content = line.strip_suffix(b"\r").unwrap_or(line);
            assert!(content.len() <= 76);
        }
    }
}
