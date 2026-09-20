//! Публичная точка запуска reactor-а с постоянным исходящим scheduler-ом.

use super::{TlsMode, server_loop};
use rustls::ServerConfig;
use std::{io, path::Path, sync::Arc};

/// Запускает все listener-ы и постоянный исходящий scheduler в одном reactor-е.
#[allow(clippy::too_many_arguments)]
pub fn serve_all_mail_outbound_starttls(
    smtp_address: &str,
    imap_address: &str,
    pop3_address: &str,
    managesieve_address: &str,
    outbound_address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    server_loop::serve(
        Some(smtp_address),
        Some(imap_address),
        Some(pop3_address),
        Some(managesieve_address),
        Some(outbound_address),
        root,
        TlsMode::StartTls(tls_config),
    )
}
