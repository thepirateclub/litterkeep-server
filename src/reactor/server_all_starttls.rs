//! Композиция входящих SMTP, IMAP и POP3 STARTTLS listener-ов.

use super::*;

/// Запускает три протокола в одном poll-reactor и с единственным Store owner.
///
/// Общий владелец состояния сохраняет порядок операций между сессиями, а один
/// reactor обслуживает все сокеты без отдельного потока на каждое соединение.
pub fn serve_all_starttls(
    smtp_address: &str,
    imap_address: &str,
    pop3_address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    server_loop::serve(
        Some(smtp_address),
        Some(imap_address),
        Some(pop3_address),
        None,
        None,
        root,
        TlsMode::StartTls(tls_config),
    )
}
