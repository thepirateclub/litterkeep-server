//! Формирование RFC 3464 Delivery Status Notification без доступа к Store.
//!
//! Этот модуль получает только уже принятый envelope sender, окончательный
//! результат delivery и immutable исходное сообщение. Он не открывает сокет,
//! не пишет файл и не ставит новое outbound job, поэтому пустой reverse-path
//! гарантированно не образует DSN-цикл.

/// Окончательная причина, показываемая локальному отправителю.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    PermanentSmtp,
    RetryExhausted,
}

/// Строит multipart/report для одного окончательно недоставленного recipient.
///
/// Пустой reverse-path обозначает DSN или auto-reply. Для него RFC-профиль
/// запрещает новый DSN, поэтому результат отсутствует до выделения памяти.
pub fn render(
    reverse_path: &str,
    recipient: &str,
    transaction_id: u64,
    original: &[u8],
    failure: Failure,
) -> Option<Vec<u8>> {
    match reverse_path.is_empty() {
        true => None,
        false => Some(render_non_empty(
            reverse_path,
            recipient,
            transaction_id,
            original,
            failure,
        )),
    }
}

/// Собирает body только после явного запрета bounce-loop в публичной границе.
fn render_non_empty(
    reverse_path: &str,
    recipient: &str,
    transaction_id: u64,
    original: &[u8],
    failure: Failure,
) -> Vec<u8> {
    let boundary = boundary(transaction_id, original);
    let status = status(failure);
    let explanation = explanation(failure);
    let mut report = Vec::with_capacity(original.len().saturating_add(768));
    append(
        &mut report,
        &format!(
            "From: MAILER-DAEMON <postmaster@litterkeep.test>\r\n\
             To: <{reverse_path}>\r\n\
             Subject: Delivery Status Notification (Failure)\r\n\
             Auto-Submitted: auto-replied\r\n\
             MIME-Version: 1.0\r\n\
             Content-Type: multipart/report; report-type=delivery-status; boundary=\"{boundary}\"\r\n\
             \r\n\
             --{boundary}\r\n\
             Content-Type: text/plain; charset=utf-8\r\n\
             \r\n\
             Delivery to <{recipient}> failed: {explanation}.\r\n\
             \r\n\
             --{boundary}\r\n\
             Content-Type: message/delivery-status\r\n\
             \r\n\
             Reporting-MTA: dns; litterkeep.test\r\n\
             Final-Recipient: rfc822; <{recipient}>\r\n\
             Action: failed\r\n\
             Status: {status}\r\n\
             \r\n\
             --{boundary}\r\n\
             Content-Type: message/rfc822\r\n\
             \r\n"
        ),
    );
    report.extend_from_slice(original);
    append(&mut report, &format!("\r\n--{boundary}--\r\n"));
    report
}

/// Выбирает ограниченный boundary, отсутствующий в возвращаемом original.
///
/// Счётчик не является источником секрета, а служит только устранению MIME
/// delimiter collision. После 32 попыток используется последний вариант:
/// вероятность коллизии остаётся диагностируемой, но не приводит к циклу.
fn boundary(transaction_id: u64, original: &[u8]) -> String {
    for attempt in 0_u8..32 {
        let candidate = format!("litterkeep-dsn-{transaction_id:x}-{attempt:x}");
        let marker = format!("--{candidate}");
        if !contains(original, marker.as_bytes()) {
            return candidate;
        }
    }
    format!("litterkeep-dsn-{transaction_id:x}-terminal")
}

/// Ищет bytes без выделений и без непосредственного индексирования массива.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    match needle.is_empty() {
        true => true,
        false => haystack
            .windows(needle.len())
            .any(|window| window == needle),
    }
}

/// Добавляет UTF-8 protocol fields как байты без изменения исходного сообщения.
fn append(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(value.as_bytes());
}

/// RFC enhanced status остаётся typed решением delivery layer.
fn status(failure: Failure) -> &'static str {
    match failure {
        Failure::PermanentSmtp => "5.0.0",
        Failure::RetryExhausted => "4.4.0",
    }
}

/// Короткий human-readable diagnostic не выдаёт сырой ответ внешнего peer-а.
fn explanation(failure: Failure) -> &'static str {
    match failure {
        Failure::PermanentSmtp => "permanent SMTP failure",
        Failure::RetryExhausted => "retry budget exhausted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_reverse_path_does_not_create_a_bounce() {
        assert_eq!(
            render("", "bob@peer.test", 7, b"body", Failure::PermanentSmtp),
            None
        );
    }

    #[test]
    fn report_preserves_original_message_bytes() {
        let original = b"Subject: source\r\n\r\npayload\x00";
        let report = render(
            "alice@litterkeep.test",
            "bob@peer.test",
            7,
            original,
            Failure::PermanentSmtp,
        )
        .expect("report");
        assert!(
            report
                .windows(original.len())
                .any(|window| window == original)
        );
        assert!(
            report
                .windows(b"report-type=delivery-status".len())
                .any(|window| { window == b"report-type=delivery-status" })
        );
        assert!(
            report
                .windows(b"Action: failed".len())
                .any(|window| window == b"Action: failed")
        );
    }

    #[test]
    fn boundary_moves_away_from_source_delimiter() {
        let source = b"--litterkeep-dsn-7-0\r\nbody";
        let report = render(
            "alice@litterkeep.test",
            "bob@peer.test",
            7,
            source,
            Failure::RetryExhausted,
        )
        .expect("report");
        assert!(
            report
                .windows(b"litterkeep-dsn-7-1".len())
                .any(|window| { window == b"litterkeep-dsn-7-1" })
        );
    }
}
