//! Вычисляет неизменяемые протокольные свойства сообщения при открытии maildrop.

/// Снимок размеров, кодировки и MIME-политики одного POP3-сообщения.
#[derive(Clone, Copy)]
pub(super) struct MessageProfile {
    pub(super) encoding: MessageEncoding,
    pub(super) octets: usize,
    pub(super) transfer_policy: MimeTransferPolicy,
}

/// Классификация октетов сообщения относительно режима POP3 UTF8.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MessageEncoding {
    /// Все октеты ASCII, поэтому сообщение допустимо в обоих режимах.
    Ascii,
    /// Сообщение содержит только корректные UTF-8 последовательности.
    Utf8,
    /// Есть не-ASCII октеты без распознанной UTF-8 последовательности.
    Opaque,
    /// Одновременно есть UTF-8 текст и октеты, ломающие сообщение.
    Mixed,
}

impl MessageEncoding {
    /// Объединяет требования нескольких сообщений для многострочного LIST.
    pub(super) fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::Ascii, value) | (value, Self::Ascii) => value,
            (Self::Utf8, Self::Utf8) => Self::Utf8,
            (Self::Opaque, Self::Opaque) => Self::Opaque,
            _ => Self::Mixed,
        }
    }
}

/// MIME transfer encoding, вычисленный при открытии POP3 maildrop.
#[derive(Clone, Copy)]
pub(crate) enum MimeTransferPolicy {
    Allowed,
    Binary,
    Unverifiable,
}

impl MessageProfile {
    /// Строит содержимое профиля один раз, до обработки сетевых команд.
    pub(super) fn new(bytes: &[u8]) -> Self {
        Self {
            encoding: Self::classify(bytes),
            octets: Self::reported_size(bytes),
            transfer_policy: Self::transfer_policy(bytes),
        }
    }

    /// Не объявляет произвольные high-bit октеты корректным UTF-8.
    pub(super) fn classify(bytes: &[u8]) -> MessageEncoding {
        match (
            bytes.iter().all(u8::is_ascii),
            contains_non_ascii_utf8(bytes),
            std::str::from_utf8(bytes).is_ok(),
        ) {
            (true, _, _) => MessageEncoding::Ascii,
            (false, true, true) => MessageEncoding::Utf8,
            (false, true, false) => MessageEncoding::Mixed,
            (false, false, _) => MessageEncoding::Opaque,
        }
    }

    /// Отказывает при непроверяемой MIME-структуре, не предполагая безопасность.
    pub(super) fn transfer_policy(bytes: &[u8]) -> MimeTransferPolicy {
        match crate::mime::tree::parse(bytes) {
            Ok(entity) => match entity.contains_binary_transfer_encoding() {
                true => MimeTransferPolicy::Binary,
                false => MimeTransferPolicy::Allowed,
            },
            Err(_) => MimeTransferPolicy::Unverifiable,
        }
    }

    /// Считает размер после CRLF-нормализации без удвоения dot-stuffing.
    pub(super) fn reported_size(bytes: &[u8]) -> usize {
        bytes
            .split_inclusive(|byte| *byte == b'\n')
            .fold(0_usize, |total, line| {
                let content = match line.strip_suffix(b"\n") {
                    Some(without_lf) => without_lf.strip_suffix(b"\r").unwrap_or(without_lf),
                    None => line,
                };
                total.saturating_add(content.len()).saturating_add(2)
            })
    }
}

/// Находит UTF-8 scalar вне ASCII, пропуская некорректные октеты без panic.
fn contains_non_ascii_utf8(bytes: &[u8]) -> bool {
    let mut remaining = bytes;
    loop {
        match std::str::from_utf8(remaining) {
            Ok(text) => return !text.is_ascii(),
            Err(error) => {
                let valid_length = error.valid_up_to();
                let prefix = remaining.get(..valid_length).unwrap_or_default();
                if !prefix.is_ascii() {
                    return true;
                }
                let invalid_length = error
                    .error_len()
                    .unwrap_or_else(|| remaining.len().saturating_sub(valid_length));
                let next = valid_length.saturating_add(invalid_length);
                remaining = remaining.get(next..).unwrap_or_default();
            }
        }
    }
}
