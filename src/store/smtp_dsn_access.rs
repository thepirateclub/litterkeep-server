//! Read-only доступ к durable SMTP DSN метаданным.

use crate::store::Store;

impl Store {
    /// Проверяет наличие сохранённого envelope без смешения UID и transaction ID.
    #[cfg(test)]
    pub(crate) fn contains_smtp_dsn_envelope(&self, expected: &crate::smtp::DsnEnvelope) -> bool {
        self.smtp_dsn_envelopes
            .values()
            .any(|envelope| envelope == expected)
    }
}
