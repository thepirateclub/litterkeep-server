//! Однопроходное декодирование MIME quoted-printable.

/// Декодирует байты за O(n), не выделяя память по заявленным размерам.
pub fn decode(input: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut output = Vec::with_capacity(input.len());
    let mut cursor = 0_usize;
    while let Some(byte) = input.get(cursor).copied() {
        match byte {
            b'=' => {
                cursor = decode_escape(input, cursor, &mut output)?;
            }
            b'\r' => {
                let Some(b'\n') = input.get(cursor.saturating_add(1)).copied() else {
                    return Err("bare CR in quoted-printable");
                };
                output.extend_from_slice(b"\r\n");
                cursor = cursor.saturating_add(2);
            }
            b'\n' => return Err("bare LF in quoted-printable"),
            b' ' | b'\t' if is_transport_padding(input, cursor) => {
                cursor = cursor.saturating_add(1);
            }
            _ => {
                output.push(byte);
                cursor = cursor.saturating_add(1);
            }
        }
    }
    Ok(output)
}

/// Определяет, принадлежит ли пробел конечной транспортной padding-последовательности.
fn is_transport_padding(input: &[u8], cursor: usize) -> bool {
    let mut lookahead = cursor;
    while matches!(input.get(lookahead), Some(b' ' | b'\t')) {
        lookahead = lookahead.saturating_add(1);
    }
    matches!(input.get(lookahead), None | Some(b'\r' | b'\n'))
}

fn decode_escape(input: &[u8], cursor: usize, output: &mut Vec<u8>) -> Result<usize, &'static str> {
    let first = input
        .get(cursor.saturating_add(1))
        .copied()
        .ok_or("truncated quoted-printable escape")?;
    let second = input
        .get(cursor.saturating_add(2))
        .copied()
        .ok_or("truncated quoted-printable escape")?;
    match (first, second) {
        (b'\r', b'\n') => Ok(cursor.saturating_add(3)),
        _ => {
            let high = hex(first).ok_or("invalid quoted-printable hex")?;
            let low = hex(second).ok_or("invalid quoted-printable hex")?;
            output.push((high << 4) | low);
            Ok(cursor.saturating_add(3))
        }
    }
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_octets_and_soft_breaks() {
        assert_eq!(
            decode(b"hello=20world=0D=0Asoft=\r\nbreak").expect("quoted-printable"),
            b"hello world\r\nsoftbreak"
        );
    }

    #[test]
    fn rejects_truncated_escape_and_bare_newline() {
        assert!(decode(b"bad=").is_err());
        assert!(decode(b"bad\nline").is_err());
    }

    #[test]
    fn removes_only_trailing_space_and_tab_from_each_encoded_line() {
        assert_eq!(
            decode(b"inside space\tkept  \r\nnext\t\r\nend ").unwrap(),
            b"inside space\tkept\r\nnext\r\nend"
        );
    }

    #[test]
    fn encoded_space_at_line_end_survives_transport_padding_trim() {
        assert_eq!(decode(b"visible=20\r\nnext").unwrap(), b"visible \r\nnext");
    }

    #[test]
    fn preserves_encoded_space_before_transport_padding() {
        assert_eq!(decode(b"visible=20 \r\nnext").unwrap(), b"visible \r\nnext");
    }
}
