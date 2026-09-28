//! Bounded payload одной локальной SMTP delivery транзакции.

/// Письмо и envelope metadata для единственного Store owner.
#[derive(Debug)]
pub struct SmtpDeliveryRequest {
    pub request_id: u64,
    pub mailboxes: Vec<String>,
    /// SMTP RCPT TO paths в том же порядке, что и mailboxes.
    pub envelope_recipients: Vec<String>,
    /// SMTP MAIL FROM до trace-заголовков, отдельно от MIME `From`.
    pub envelope_sender: String,
    pub dsn: crate::smtp::DsnEnvelope,
    pub bytes: Vec<u8>,
}
