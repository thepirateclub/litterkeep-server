//! Безопасность адресата и интервал повторов для RFC 5230 vacation.

const MIN_DAYS: u64 = 1;
const MAX_DAYS: u64 = 365;
const DEFAULT_DAYS: u64 = 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SuppressionHeader {
    List,
    AutoSubmitted,
    Other,
}

/// Применяет локальную days policy: min > 0, max > 7 и bounded storage.
pub(crate) fn effective_days(requested: Option<u64>) -> u64 {
    requested.unwrap_or(DEFAULT_DAYS).clamp(MIN_DAYS, MAX_DAYS)
}

/// Подавляет ответы на пустой/системный envelope sender и list/auto mail.
pub(crate) fn may_respond(sender: &str, headers: &[(String, String)]) -> bool {
    if sender.is_empty() || automated_sender(sender) {
        return false;
    }
    !headers
        .iter()
        .any(|(name, value)| match suppression_header(name) {
            SuppressionHeader::List => true,
            SuppressionHeader::AutoSubmitted => !value.trim().eq_ignore_ascii_case("no"),
            SuppressionHeader::Other => false,
        })
}

/// Ограничивает ответы mailbox-адресами из `:addresses`, если список задан.
pub(crate) fn recipient_allowed(addresses: &[String], recipient: &str) -> bool {
    if addresses.is_empty() {
        return true;
    }
    let Ok(targets) = crate::mime::envelope_address::parse_value(recipient) else {
        return false;
    };
    let Some(target) = targets.first() else {
        return false;
    };
    addresses.iter().any(|configured| {
        match crate::mime::envelope_address::parse_value(configured) {
            Ok(values) => values.first().is_some_and(|address| {
                matches!(
                    (
                        address.local.eq_ignore_ascii_case(&target.local),
                        address.domain.eq_ignore_ascii_case(&target.domain),
                    ),
                    (true, true)
                )
            }),
            Err(_) => false,
        }
    })
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
