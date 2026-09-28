//! Сериализация MIME-отчёта RFC 3464 с параметрами RFC 3461.

use crate::{
    dsn::Failure,
    queue::DsnDeliveryMetadata,
    smtp::{OriginalAddressType, ReturnContent},
};

/// Формирует multipart/report для одного окончательно недоставленного RCPT.
pub(super) fn render(
    reverse_path: &str,
    recipient: &str,
    transaction_id: u64,
    original: &[u8],
    failure: Failure,
    metadata: Option<&DsnDeliveryMetadata>,
) -> Vec<u8> {
    let returned = returned_content(original, metadata);
    let boundary = boundary(transaction_id, returned);
    let status_type = delivery_status_type(recipient, metadata);
    let message_type = returned_message_type(returned, metadata);
    let final_recipient_type = final_recipient_type(recipient);
    let mut output = Vec::with_capacity(returned.len().saturating_add(1024));
    append(
        &mut output,
        &format!(
            "From: MAILER-DAEMON <postmaster@litterkeep.test>\r\nTo: <{reverse_path}>\r\n\
         Subject: Delivery Status Notification (Failure)\r\nAuto-Submitted: auto-replied\r\n\
         MIME-Version: 1.0\r\nContent-Type: multipart/report; report-type=delivery-status; boundary=\"{boundary}\"\r\n\r\n\
         --{boundary}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n\
         Delivery to <{recipient}> failed: {}.\r\n\r\n--{boundary}\r\n\
         Content-Type: {status_type}\r\n\r\nReporting-MTA: dns; litterkeep.test\r\n{}\r\n\
         {}Final-Recipient: {final_recipient_type}; {recipient}\r\nAction: failed\r\nStatus: {}\r\n\r\n\
         --{boundary}\r\nContent-Type: {message_type}\r\n\r\n",
            explanation(failure),
            envelope_id(metadata),
            original_recipient(metadata),
            status(failure)
        ),
    );
    output.extend_from_slice(returned);
    append(&mut output, &format!("\r\n--{boundary}--\r\n"));
    output
}

/// Ограничивает RET=HDRS первым MIME-разделителем и не возвращает тело.
fn returned_content<'a>(original: &'a [u8], metadata: Option<&DsnDeliveryMetadata>) -> &'a [u8] {
    let headers_only = metadata
        .and_then(|value| value.mail.return_content)
        .is_some_and(|value| value == ReturnContent::Headers);
    match headers_only {
        true => original
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .and_then(|position| original.get(..position.saturating_add(4)))
            .unwrap_or(&[]),
        false => original,
    }
}

/// Формирует Orig-Recipient из сохранённого типизированного ORCPT.
fn original_recipient(metadata: Option<&DsnDeliveryMetadata>) -> String {
    let Some(value) = metadata
        .and_then(|item| item.recipient.as_ref())
        .and_then(|item| item.original_recipient.as_ref())
    else {
        return String::new();
    };
    let address_type = match value.address_type {
        OriginalAddressType::Rfc822 => "rfc822",
        OriginalAddressType::Utf8 => "utf-8",
    };
    format!(
        "Original-Recipient: {address_type}; {}\r\n",
        String::from_utf8_lossy(&value.address)
    )
}

/// Возвращает ENVID в виде декодированного RFC 3461 opaque value.
fn envelope_id(metadata: Option<&DsnDeliveryMetadata>) -> String {
    metadata
        .and_then(|value| value.mail.envelope_id.as_ref())
        .map(|value| {
            format!(
                "Original-Envelope-ID: {}\r\n",
                String::from_utf8_lossy(value)
            )
        })
        .unwrap_or_default()
}

/// Выбирает глобальный DSN subtype, если адреса требуют UTF-8.
fn delivery_status_type(recipient: &str, metadata: Option<&DsnDeliveryMetadata>) -> &'static str {
    let original_is_utf8 = metadata
        .and_then(|value| value.recipient.as_ref())
        .and_then(|value| value.original_recipient.as_ref())
        .is_some_and(|value| value.address_type == OriginalAddressType::Utf8);
    match original_is_utf8 || !recipient.is_ascii() {
        true => "message/global-delivery-status",
        false => "message/delivery-status",
    }
}

/// Сериализует конечный адрес как ASCII mailbox или UTF-8 mailbox.
fn final_recipient_type(recipient: &str) -> &'static str {
    match recipient.is_ascii() {
        true => "rfc822",
        false => "utf-8",
    }
}

/// Выбирает обычный или SMTPUTF8 media type для возвращённых заголовков.
fn returned_message_type(returned: &[u8], metadata: Option<&DsnDeliveryMetadata>) -> &'static str {
    let utf8_headers = header_bytes(returned).iter().any(|byte| !byte.is_ascii());
    let headers_only = metadata
        .and_then(|value| value.mail.return_content)
        .is_some_and(|value| value == ReturnContent::Headers);
    match (headers_only, utf8_headers) {
        (true, true) => "message/global-headers",
        (true, false) => "text/rfc822-headers",
        (false, true) => "message/global",
        (false, false) => "message/rfc822",
    }
}

/// Возвращает заголовочную часть, не включая пустую строку и тело.
fn header_bytes(message: &[u8]) -> &[u8] {
    message
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .and_then(|position| message.get(..position))
        .unwrap_or(message)
}

/// Выбирает безопасную для пользователя диагностическую фразу.
fn explanation(failure: Failure) -> &'static str {
    match failure {
        Failure::PermanentSmtp => "permanent SMTP failure",
        Failure::RoutePolicy => "recipient route denied by policy",
        Failure::RetryExhausted => "retry budget exhausted",
    }
}

/// Сопоставляет конечную причину с enhanced status code.
fn status(failure: Failure) -> &'static str {
    match failure {
        Failure::PermanentSmtp => "5.0.0",
        Failure::RoutePolicy => "5.7.1",
        Failure::RetryExhausted => "4.4.0",
    }
}

/// Выбирает boundary, отсутствующий в фактически возвращаемой части письма.
fn boundary(transaction_id: u64, original: &[u8]) -> String {
    for attempt in 0_u8..32 {
        let candidate = format!("litterkeep-dsn-{transaction_id:x}-{attempt:x}");
        if !contains(original, format!("--{candidate}").as_bytes()) {
            return candidate;
        }
    }
    format!("litterkeep-dsn-{transaction_id:x}-terminal")
}

/// Ищет последовательность байтов без индексации и дополнительных буферов.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty()
        || haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// Добавляет уже проверенный текстовый фрагмент в байтовый MIME-буфер.
fn append(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smtp::{MailDsn, OriginalRecipient, RecipientDsn};

    /// Проверяет, что sidecar отражается в обоих DSN-блоках по RFC 3461.
    #[test]
    fn report_contains_original_envelope_and_recipient() {
        let metadata = DsnDeliveryMetadata {
            mail: MailDsn {
                envelope_id: Some(b"batch 42".to_vec()),
                ..MailDsn::default()
            },
            recipient: Some(RecipientDsn {
                original_recipient: Some(OriginalRecipient {
                    address_type: OriginalAddressType::Rfc822,
                    address: b"alias@peer.test".to_vec(),
                }),
                ..RecipientDsn::default()
            }),
        };
        let report = render(
            "alice@litterkeep.test",
            "final@peer.test",
            17,
            b"Subject: original\r\n\r\nbody",
            Failure::PermanentSmtp,
            Some(&metadata),
        );
        assert!(contains(&report, b"Original-Envelope-ID: batch 42\r\n"));
        assert!(contains(
            &report,
            b"Original-Recipient: rfc822; alias@peer.test\r\n"
        ));
        assert!(contains(
            &report,
            b"Original-Envelope-ID: batch 42\r\n\r\nOriginal-Recipient: rfc822; alias@peer.test\r\nFinal-Recipient: rfc822; final@peer.test"
        ));
    }

    /// Проверяет, что RET=HDRS никогда не включает исходное тело.
    #[test]
    fn headers_return_policy_excludes_the_message_body() {
        let metadata = DsnDeliveryMetadata {
            mail: MailDsn {
                return_content: Some(ReturnContent::Headers),
                ..MailDsn::default()
            },
            recipient: None,
        };
        let source = b"Subject: secret subject\r\nX-Trace: one\r\n\r\nsecret body bytes";
        let report = render(
            "alice@litterkeep.test",
            "bob@peer.test",
            18,
            source,
            Failure::PermanentSmtp,
            Some(&metadata),
        );
        assert!(contains(
            &report,
            b"Subject: secret subject\r\nX-Trace: one\r\n\r\n"
        ));
        assert!(contains(&report, b"Content-Type: text/rfc822-headers"));
        assert!(!contains(&report, b"secret body bytes"));
        let malformed = render(
            "alice@litterkeep.test",
            "bob@peer.test",
            21,
            b"unseparated secret bytes",
            Failure::PermanentSmtp,
            Some(&metadata),
        );
        assert!(!contains(&malformed, b"unseparated secret bytes"));
    }

    /// RFC 6533 использует global-headers для заголовков с Unicode.
    #[test]
    fn international_headers_use_the_global_headers_media_type() {
        let metadata = DsnDeliveryMetadata {
            mail: MailDsn {
                return_content: Some(ReturnContent::Headers),
                ..MailDsn::default()
            },
            recipient: None,
        };
        let report = render(
            "alice@litterkeep.test",
            "bob@peer.test",
            20,
            "Subject: Привет\r\n\r\nbody".as_bytes(),
            Failure::PermanentSmtp,
            Some(&metadata),
        );
        assert!(contains(&report, b"Content-Type: message/global-headers"));
        assert!(!contains(&report, b"body"));
    }

    /// RFC 6533 ORCPT сохраняет тип utf-8, а не преобразуется в rfc822.
    #[test]
    fn utf8_original_recipient_keeps_its_registered_address_type() {
        let metadata = DsnDeliveryMetadata {
            mail: MailDsn::default(),
            recipient: Some(RecipientDsn {
                original_recipient: Some(OriginalRecipient {
                    address_type: OriginalAddressType::Utf8,
                    address: "用户@例.test".as_bytes().to_vec(),
                }),
                ..RecipientDsn::default()
            }),
        };
        let report = render(
            "alice@litterkeep.test",
            "user@例.test",
            19,
            b"Subject: original\r\n\r\nbody",
            Failure::PermanentSmtp,
            Some(&metadata),
        );
        assert!(contains(
            &report,
            "Original-Recipient: utf-8; 用户@例.test\r\n".as_bytes()
        ));
        assert!(contains(
            &report,
            b"Content-Type: message/global-delivery-status"
        ));
        assert!(contains(
            &report,
            "Final-Recipient: utf-8; user@例.test\r\n".as_bytes()
        ));
    }
}
