//! Выбор MIME-содержимого, возвращаемого в отчёте DSN.

use crate::{
    dsn::ReportOutcome,
    queue::DsnDeliveryMetadata,
    smtp::{OriginalAddressType, ReturnContent},
};

/// Возвращает сообщения подходящий MIME тип для RET и SMTPUTF8.
pub(super) fn returned_message_type(
    returned: &[u8],
    outcome: ReportOutcome,
    metadata: Option<&DsnDeliveryMetadata>,
) -> &'static str {
    let headers_only = match outcome {
        ReportOutcome::Delivered | ReportOutcome::Relayed => true,
        ReportOutcome::Failure(_) => metadata
            .and_then(|value| value.mail.return_content)
            .is_some_and(|value| value == ReturnContent::Headers),
    };
    let utf8_headers = header_bytes(returned).iter().any(|byte| !byte.is_ascii())
        || metadata.is_some_and(metadata_uses_utf8);
    match (headers_only, utf8_headers) {
        (true, true) => "message/global-headers",
        (true, false) => "text/rfc822-headers",
        (false, true) => "message/global",
        (false, false) => "message/rfc822",
    }
}

/// Возвращает заголовки без разделителя, чтобы выбрать MIME subtype.
fn header_bytes(message: &[u8]) -> &[u8] {
    message
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .and_then(|position| message.get(..position))
        .unwrap_or(message)
}

/// Проверяет необходимость UTF-8 заголовков по origin и конечному RCPT.
fn metadata_uses_utf8(metadata: &DsnDeliveryMetadata) -> bool {
    metadata.origin_recipients.iter().any(|recipient| {
        recipient
            .as_ref()
            .and_then(|value| value.original_recipient.as_ref())
            .is_some_and(|value| value.address_type == OriginalAddressType::Utf8)
    }) || metadata
        .recipient
        .as_ref()
        .and_then(|value| value.original_recipient.as_ref())
        .is_some_and(|value| value.address_type == OriginalAddressType::Utf8)
}

/// Ищет граничную строку линейным окном без копирования полного заголовка.
pub(super) fn returned_content_contains(haystack: &[u8], needle: &[u8]) -> bool {
    match needle.is_empty() {
        true => true,
        false => haystack
            .windows(needle.len())
            .any(|window| window == needle),
    }
}
