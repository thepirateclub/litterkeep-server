//! Сериализация MIME-отчёта RFC 3464 с параметрами RFC 3461.

use crate::{
    dsn::{Failure, ReportOutcome},
    queue::DsnDeliveryMetadata,
    smtp::{OriginalAddressType, ReturnContent},
};

/// Формирует multipart/report для одного окончательно недоставленного RCPT.
pub(super) fn render(
    reverse_path: &str,
    recipient: &str,
    transaction_id: u64,
    original: &[u8],
    outcome: ReportOutcome,
    metadata: Option<&DsnDeliveryMetadata>,
) -> Vec<u8> {
    let returned = returned_content(original, outcome, metadata);
    let boundary = boundary(transaction_id, returned);
    let status_type = delivery_status_type(recipient, metadata);
    let message_type = returned_message_type(returned, outcome, metadata);
    let (subject, summary, action, status) = report_fields(outcome);
    let mut output = Vec::with_capacity(returned.len().saturating_add(1024));
    append(
        &mut output,
        &format!(
            "From: MAILER-DAEMON <postmaster@litterkeep.test>\r\nTo: <{reverse_path}>\r\n\
         Subject: Delivery Status Notification ({subject})\r\nAuto-Submitted: auto-replied\r\n\
         MIME-Version: 1.0\r\nContent-Type: multipart/report; report-type=delivery-status; boundary=\"{boundary}\"\r\n\r\n\
         --{boundary}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n\
         {summary} <{recipient}>.\r\n\r\n--{boundary}\r\n\
         Content-Type: {status_type}\r\n\r\nReporting-MTA: dns; litterkeep.test\r\n{}\r\n",
            envelope_id(metadata),
        ),
    );
    append_recipient_groups(&mut output, recipient, action, status, metadata, outcome);
    append(
        &mut output,
        &format!("--{boundary}\r\nContent-Type: {message_type}\r\n\r\n"),
    );
    output.extend_from_slice(returned);
    append(&mut output, &format!("\r\n--{boundary}--\r\n"));
    output
}

/// Ограничивает RET=HDRS первым MIME-разделителем и не возвращает тело.
fn returned_content<'a>(
    original: &'a [u8],
    outcome: ReportOutcome,
    metadata: Option<&DsnDeliveryMetadata>,
) -> &'a [u8] {
    let headers_only = match outcome {
        ReportOutcome::Delivered => true,
        ReportOutcome::Failure(_) => metadata
            .and_then(|value| value.mail.return_content)
            .is_some_and(|value| value == ReturnContent::Headers),
    };
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
fn original_recipient(recipient: Option<&crate::smtp::RecipientDsn>) -> String {
    let Some(value) = recipient.and_then(|item| item.original_recipient.as_ref()) else {
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

/// Сериализует отдельную recipient-группу для каждого исходного SMTP RCPT.
fn append_recipient_groups(
    output: &mut Vec<u8>,
    recipient: &str,
    action: &str,
    status: &str,
    metadata: Option<&DsnDeliveryMetadata>,
    outcome: ReportOutcome,
) {
    let Some(metadata) = metadata else {
        append_recipient_group(output, recipient, None, action, status);
        return;
    };
    match metadata.origin_recipients.is_empty() {
        true => append_recipient_group(
            output,
            recipient,
            metadata.recipient.as_ref(),
            action,
            status,
        ),
        false => metadata
            .origin_recipients
            .iter()
            .filter(|origin| origin_notification_allowed(origin.as_ref(), outcome))
            .for_each(|origin| {
                append_recipient_group(output, recipient, origin.as_ref(), action, status)
            }),
    }
}

/// Применяет NOTIFY конкретного RCPT к результату формируемого DSN.
fn origin_notification_allowed(
    recipient: Option<&crate::smtp::RecipientDsn>,
    outcome: ReportOutcome,
) -> bool {
    let condition = match outcome {
        ReportOutcome::Delivered => crate::smtp::NotifyCondition::Success,
        ReportOutcome::Failure(_) => crate::smtp::NotifyCondition::Failure,
    };
    crate::dsn::notification_allowed_for_recipient(recipient, condition)
}

/// Добавляет полный Recipient block без слияния сведений двух alias-ов.
fn append_recipient_group(
    output: &mut Vec<u8>,
    recipient: &str,
    origin: Option<&crate::smtp::RecipientDsn>,
    action: &str,
    status: &str,
) {
    let recipient_type = final_recipient_type(recipient);
    append(output, &original_recipient(origin));
    append(
        output,
        &format!(
            "Final-Recipient: {recipient_type}; {recipient}\r\nAction: {action}\r\nStatus: {status}\r\n\r\n"
        ),
    );
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
    let original_is_utf8 = metadata.is_some_and(|value| {
        value.origin_recipients.iter().any(|recipient| {
            recipient
                .as_ref()
                .and_then(|value| value.original_recipient.as_ref())
                .is_some_and(|value| value.address_type == OriginalAddressType::Utf8)
        }) || value
            .recipient
            .as_ref()
            .and_then(|value| value.original_recipient.as_ref())
            .is_some_and(|value| value.address_type == OriginalAddressType::Utf8)
    });
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
fn returned_message_type(
    returned: &[u8],
    outcome: ReportOutcome,
    metadata: Option<&DsnDeliveryMetadata>,
) -> &'static str {
    let utf8_headers = header_bytes(returned).iter().any(|byte| !byte.is_ascii());
    let headers_only = match outcome {
        ReportOutcome::Delivered => true,
        ReportOutcome::Failure(_) => metadata
            .and_then(|value| value.mail.return_content)
            .is_some_and(|value| value == ReturnContent::Headers),
    };
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

/// Возвращает согласованные поля текста, Action и Status для outcome.
fn report_fields(
    outcome: ReportOutcome,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match outcome {
        ReportOutcome::Delivered => (
            "Success",
            "Your message was successfully delivered to",
            "delivered",
            "2.0.0",
        ),
        ReportOutcome::Failure(failure) => match failure {
            Failure::PermanentSmtp => ("Failure", "Delivery failed for", "failed", "5.0.0"),
            Failure::RoutePolicy => (
                "Failure",
                "Delivery was denied by recipient policy for",
                "failed",
                "5.7.1",
            ),
            Failure::RetryExhausted => (
                "Failure",
                "Delivery retry budget was exhausted for",
                "failed",
                "4.4.0",
            ),
        },
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
#[path = "dsn_report_tests.rs"]
mod tests;
