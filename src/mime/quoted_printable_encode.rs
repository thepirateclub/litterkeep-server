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
                append_token(
                    &mut output,
                    &mut line_length,
                    format!("={byte:02X}").as_bytes(),
                );
                cursor = cursor.saturating_add(1);
            }
            b'!'..=b'<' | b'>'..=b'~' => {
                append_token(&mut output, &mut line_length, &[byte]);
                cursor = cursor.saturating_add(1);
            }
            _ => {
                let token = format!("={byte:02X}");
                append_token(&mut output, &mut line_length, token.as_bytes());
                cursor = cursor.saturating_add(1);
            }
        }
    }
    output
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
