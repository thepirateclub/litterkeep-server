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
    let route = crate::outbound_route::Route::plain(outbound_address)?;
    server_loop::serve(
        Some(smtp_address),
        Some(imap_address),
        Some(pop3_address),
        Some(managesieve_address),
        Some(route),
        root,
        TlsMode::StartTls(tls_config),
    )
}

/// Запускает тот же scheduler с отдельным implicit-TLS route context.
#[allow(clippy::too_many_arguments)]
pub fn serve_all_mail_outbound_tls_starttls(
    smtp_address: &str,
    imap_address: &str,
    pop3_address: &str,
    managesieve_address: &str,
    outbound_address: &str,
    outbound_server_name: &str,
    outbound_ca_der: Vec<u8>,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    let route = crate::outbound_route::Route::implicit_tls(
        outbound_address,
        outbound_server_name,
        outbound_ca_der,
    )?;
    server_loop::serve(
        Some(smtp_address),
        Some(imap_address),
        Some(pop3_address),
        Some(managesieve_address),
        Some(route),
        root,
        TlsMode::StartTls(tls_config),
    )
}
