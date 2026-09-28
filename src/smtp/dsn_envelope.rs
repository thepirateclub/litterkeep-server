//! Проверка наличия запрошенных отправителем RFC 3461 параметров.

use super::{DsnEnvelope, MailDsn, RecipientDsn};

impl DsnEnvelope {
    /// Отделяет обычную SMTP доставку от envelope с DSN policy.
    pub fn has_dsn_parameters(&self) -> bool {
        let mail_has_dsn = self.mail != MailDsn::default();
        let recipient_has_dsn = self
            .recipients
            .iter()
            .any(|recipient| recipient.dsn != RecipientDsn::default());
        matches!((mail_has_dsn, recipient_has_dsn), (true, _) | (_, true))
    }
}
