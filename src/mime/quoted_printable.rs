//! Однопроходное декодирование MIME quoted-printable.

/// Декодирует байты за O(n), не выделяя память по заявленным размерам.
pub fn decode(input: &[u8]) -> Result<Vec<u8>, &'static str> {
    decode_with_policy(input, InvalidEscapePolicy::Preserve)
}

/// Декодирует только полностью корректный quoted-printable объект.
pub(crate) fn decode_strict(input: &[u8]) -> Result<Vec<u8>, &'static str> {
    decode_with_policy(input, InvalidEscapePolicy::Reject)
}

/// Поведение при некорректной `=` escape-последовательности.
#[derive(Clone, Copy)]
enum InvalidEscapePolicy {
    Preserve,
    Reject,
}

/// Выполняет проход с явно выбранной политикой повреждённых escape-последовательностей.
fn decode_with_policy(input: &[u8], policy: InvalidEscapePolicy) -> Result<Vec<u8>, &'static str> {
    let mut output = Vec::with_capacity(input.len());
    let mut cursor = 0_usize;
    while let Some(byte) = input.get(cursor).copied() {
        match byte {
            b'=' => {
                cursor = decode_escape(input, cursor, &mut output, policy)?;
            }
            b'\r' => {
                let Some(b'\n') = input.get(cursor.saturating_add(1)).copied() else {
                    return Err("bare CR in quoted-printable");
                };
                output.extend_from_slice(b"\r\n");
                cursor = cursor.saturating_add(2);
            }
            b'\n' => return Err("bare LF in quoted-printable"),
            b' ' | b'\t' => {
                cursor = decode_whitespace_run(input, cursor, &mut output)?;
            }
            _ => {
                output.push(byte);
                cursor = cursor.saturating_add(1);
            }
        }
    }
    Ok(output)
}

/// Обрабатывает непрерывную группу SP/HTAB за один линейный проход.
///
/// RFC 2045 §6.7 удаляет transport padding у hard line, но сохраняет WSP перед
/// `=` soft break, поскольку этот пробел принадлежит encoded содержимому.
fn decode_whitespace_run(
    input: &[u8],
    cursor: usize,
    output: &mut Vec<u8>,
) -> Result<usize, &'static str> {
    let mut end = cursor;
    while matches!(input.get(end), Some(b' ' | b'\t')) {
        end = end.saturating_add(1);
    }
    match input.get(end) {
        None | Some(b'\r' | b'\n') => Ok(end),
        Some(_) => {
            let run = input
                .get(cursor..end)
                .ok_or("invalid quoted-printable whitespace range")?;
            output.extend_from_slice(run);
            Ok(end)
        }
    }
}

fn decode_escape(
    input: &[u8],
    cursor: usize,
    output: &mut Vec<u8>,
    policy: InvalidEscapePolicy,
) -> Result<usize, &'static str> {
    let first = input.get(cursor.saturating_add(1)).copied();
    match first {
        None => recover_escape_tail(b"=", output, cursor, 1, policy),
        Some(b'\r') => match input.get(cursor.saturating_add(2)) {
            Some(b'\n') => Ok(cursor.saturating_add(3)),
            _ => Err("bare CR in quoted-printable escape"),
        },
        Some(b'\n') => Err("bare LF in quoted-printable escape"),
        Some(first) => decode_hex_or_preserve(input, cursor, first, output, policy),
    }
}

/// Декодирует полный hex-octet либо применяет выбранное восстановление.
fn decode_hex_or_preserve(
    input: &[u8],
    cursor: usize,
    first: u8,
    output: &mut Vec<u8>,
    policy: InvalidEscapePolicy,
) -> Result<usize, &'static str> {
    let second = input.get(cursor.saturating_add(2)).copied();
    match (hex(first), second.and_then(hex)) {
        (Some(high), Some(low)) => {
            output.push((high << 4) | low);
            Ok(cursor.saturating_add(3))
        }
        _ => recover_escape_tail(&[b'=', first], output, cursor, 2, policy),
    }
}

/// Сохраняет повреждённую пару по RFC либо возвращает ошибку строгому вызывающему.
fn recover_escape_tail(
    literal: &[u8],
    output: &mut Vec<u8>,
    cursor: usize,
    consumed: usize,
    policy: InvalidEscapePolicy,
) -> Result<usize, &'static str> {
    match policy {
        InvalidEscapePolicy::Preserve => {
            output.extend_from_slice(literal);
            Ok(cursor.saturating_add(consumed))
        }
        InvalidEscapePolicy::Reject => Err("invalid quoted-printable escape"),
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
    fn recovers_invalid_escape_without_changing_its_octets() {
        assert_eq!(decode(b"bad=").unwrap(), b"bad=");
        assert_eq!(decode(b"bad=G1").unwrap(), b"bad=G1");
        assert_eq!(decode(b"bad=A").unwrap(), b"bad=A");
        assert!(decode_strict(b"bad=G1").is_err());
        assert!(decode(b"bad\nline").is_err());
        assert!(decode(b"bad=\rline").is_err());
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

    #[test]
    fn preserves_content_whitespace_before_soft_line_break() {
        assert_eq!(decode(b"visible \t=\r\nnext").unwrap(), b"visible \tnext");
        assert_eq!(decode(b"visible=20=\r\nnext").unwrap(), b"visible next");
    }

    /// Длинные WSP runs не требуют повторного сканирования suffix для каждого октета.
    #[test]
    fn processes_large_whitespace_runs_without_changing_qp_semantics() {
        let run_length = 8_192;
        let whitespace = std::iter::repeat_n(b' ', run_length);
        let mut padded = vec![b'a'];
        padded.extend(whitespace.clone());
        padded.extend_from_slice(b"\r\nb");
        assert_eq!(decode(&padded).expect("transport padding"), b"a\r\nb");

        let mut soft_break = vec![b'a'];
        soft_break.extend(whitespace.clone());
        soft_break.extend_from_slice(b"=\r\nb");
        let mut expected = vec![b'a'];
        expected.extend(whitespace);
        expected.push(b'b');
        assert_eq!(decode(&soft_break).expect("soft-break content"), expected);
    }
}
