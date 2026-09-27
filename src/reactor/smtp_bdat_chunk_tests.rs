//! Проверки exact-byte boundary для SMTP BDAT receiver.

use super::{Progress, Receiver};

/// Сохраняет binary payload и оставляет следующий SMTP command вне literal.
#[test]
fn consumes_exact_chunk_and_preserves_pipelined_command() {
    let mut receiver = Receiver::new(4, 0, 16);
    let mut input = b"\0\r\nXNOOP\r\n".to_vec();
    let mut message = Vec::new();

    assert_eq!(
        receiver.consume(&mut input, &mut message),
        Progress::Complete
    );
    assert_eq!(message, b"\0\r\nX");
    assert_eq!(input, b"NOOP\r\n");
}

/// Поддерживает chunk body, разделённый произвольными TCP reads.
#[test]
fn waits_for_remainder_before_completing_chunk() {
    let mut receiver = Receiver::new(5, 0, 16);
    let mut message = Vec::new();
    let mut first = b"ab".to_vec();
    assert_eq!(
        receiver.consume(&mut first, &mut message),
        Progress::Waiting
    );
    assert!(first.is_empty());

    let mut second = b"cdeNOOP\r\n".to_vec();
    assert_eq!(
        receiver.consume(&mut second, &mut message),
        Progress::Complete
    );
    assert_eq!(message, b"abcde");
    assert_eq!(second, b"NOOP\r\n");
}

/// Нулевой chunk завершён без поглощения следующего байта.
#[test]
fn zero_length_chunk_does_not_consume_pipeline() {
    let mut receiver = Receiver::new(0, 0, 16);
    let mut input = b"NOOP\r\n".to_vec();
    let mut message = Vec::new();

    assert_eq!(
        receiver.consume(&mut input, &mut message),
        Progress::Complete
    );
    assert_eq!(input, b"NOOP\r\n");
    assert!(message.is_empty());
}

/// Переполненный chunk полностью drain-ится, не выделяя его payload.
#[test]
fn over_limit_chunk_is_drained_without_consuming_next_command() {
    let mut receiver = Receiver::new(5, 14, 16);
    let mut input = b"12345RSET\r\n".to_vec();
    let mut message = b"existing-data".to_vec();

    assert_eq!(
        receiver.consume(&mut input, &mut message),
        Progress::TooLarge
    );
    assert_eq!(message, b"existing-data");
    assert_eq!(input, b"RSET\r\n");
}
