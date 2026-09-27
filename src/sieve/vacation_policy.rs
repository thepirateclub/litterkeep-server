//! Безопасность адресата и интервал повторов для RFC 5230 vacation.

const MIN_SECONDS: u64 = 60;
const MAX_SECONDS: u64 = 365 * 86_400;
const DEFAULT_SECONDS: u64 = 7 * 86_400;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SuppressionHeader {
    List,
    AutoSubmitted,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecipientHeader {
    To,
    Cc,
    Bcc,
    ResentTo,
    ResentCc,
    ResentBcc,
    Other,
}

/// Ограничивает интервал локальной политикой и оставляет storage bounded.
///
/// RFC 6131 разрешает серверу задавать минимальный интервал; локальный минимум
/// в одну минуту защищает очередь автоответов от циклов и всплесков нагрузки.
pub(crate) fn effective_seconds(requested: Option<super::VacationInterval>) -> u64 {
    let requested_seconds = match requested {
        Some(super::VacationInterval::Days(days)) => days.saturating_mul(86_400),
        Some(super::VacationInterval::Seconds(seconds)) => seconds,
        None => DEFAULT_SECONDS,
    };
    requested_seconds.clamp(MIN_SECONDS, MAX_SECONDS)
}

/// Подавляет ответы на пустой/системный envelope sender и list/auto mail.
pub(crate) fn may_respond(sender: &str, headers: &[(String, String)]) -> bool {
    if sender.is_empty() {
        return false;
    }
    if automated_sender(sender) {
        return false;
    }
    if headers
        .iter()
        .any(|(name, _)| matches!(suppression_header(name), SuppressionHeader::List))
    {
        return false;
    }
    let mut auto_submitted = headers
        .iter()
        .filter(|(name, _)| matches!(suppression_header(name), SuppressionHeader::AutoSubmitted))
        .map(|(_, value)| value.as_str());
    match (auto_submitted.next(), auto_submitted.next()) {
        (None, None) => true,
        (Some(value), None) => matches!(
            super::auto_submitted::parse(value),
            super::auto_submitted::Keyword::No
        ),
        _ => false,
    }
}

/// Проверяет RFC 5230 §4.5 по реальному SMTP RCPT и адресным заголовкам письма.
///
/// Final recipient уже маршрутизирован в mailbox владельца. Дополнительные
/// `:addresses` расширяют множество его адресов, но не сужают его. В исходном
/// сообщении хотя бы один такой адрес обязан присутствовать в To/Cc/Bcc либо
/// Resent-To/Resent-Cc/Resent-Bcc; произвольные address-like headers не подходят.
pub(crate) fn personal_message(
    envelope_recipient: &str,
    extra_addresses: &[String],
    headers: &[(String, String)],
) -> bool {
    let Ok(envelope_addresses) = crate::mime::envelope_address::parse_value(envelope_recipient)
    else {
        return false;
    };
    let Some(envelope_address) = envelope_addresses.first() else {
        return false;
    };
    let supplemental = extra_addresses
        .iter()
        .filter_map(|configured| crate::mime::envelope_address::parse_value(configured).ok())
        .flatten()
        .collect::<Vec<_>>();
    headers
        .iter()
        .any(|(name, value)| match recipient_header(name) {
            RecipientHeader::Other => false,
            _ => match crate::mime::envelope_address::parse_value(value) {
                Ok(addresses) => addresses.iter().any(|address| {
                    let matches_envelope = same_mailbox(address, envelope_address);
                    let matches_supplemental = supplemental
                        .iter()
                        .any(|configured| same_mailbox(address, configured));
                    matches!(
                        (matches_envelope, matches_supplemental),
                        (true, _) | (_, true)
                    )
                }),
                Err(_) => false,
            },
        })
}

/// Сопоставляет mailbox по local-part и domain без display-name и CFWS.
fn same_mailbox(
    left: &crate::mime::envelope_address::MailboxParts,
    right: &crate::mime::envelope_address::MailboxParts,
) -> bool {
    matches!(
        (
            left.local.eq_ignore_ascii_case(&right.local),
            left.domain.eq_ignore_ascii_case(&right.domain),
        ),
        (true, true)
    )
}

fn recipient_header(name: &str) -> RecipientHeader {
    match name.to_ascii_lowercase().as_str() {
        "to" => RecipientHeader::To,
        "cc" => RecipientHeader::Cc,
        "bcc" => RecipientHeader::Bcc,
        "resent-to" => RecipientHeader::ResentTo,
        "resent-cc" => RecipientHeader::ResentCc,
        "resent-bcc" => RecipientHeader::ResentBcc,
        _ => RecipientHeader::Other,
    }
}

fn suppression_header(name: &str) -> SuppressionHeader {
    match name.to_ascii_lowercase().as_str() {
        "list-id" | "list-help" | "list-subscribe" | "list-unsubscribe" | "list-post"
        | "list-owner" | "list-archive" => SuppressionHeader::List,
        "auto-submitted" => SuppressionHeader::AutoSubmitted,
        _ => SuppressionHeader::Other,
    }
}

fn automated_sender(sender: &str) -> bool {
    let Ok(addresses) = crate::mime::envelope_address::parse_value(sender) else {
        return true;
    };
    if addresses.len() != 1 {
        return true;
    }
    let Some(address) = addresses.first() else {
        return true;
    };
    let local = address.local.to_ascii_lowercase();
    match (
        matches!(local.as_str(), "mailer-daemon" | "listserv" | "majordomo"),
        local.ends_with("-request"),
        local.starts_with("owner-"),
    ) {
        (true, _, _) | (_, true, _) | (_, _, true) => true,
        (false, false, false) => false,
    }
}
