//! Завершающий CRLF для строки после client-to-server literal.

/// Продвигает bounded CRLF receiver, не забирая байты следующей команды.
pub(super) fn consume(bytes: &mut Vec<u8>, input: &mut Vec<u8>) -> Option<bool> {
    let remaining = 2usize.saturating_sub(bytes.len());
    let count = remaining.min(input.len());
    bytes.extend(input.drain(..count));
    match bytes.as_slice() {
        b"\r\n" => Some(true),
        [b'\r'] => Some(false),
        [] => Some(false),
        _ => None,
    }
}
