//! Единоличная композиция протокольных listener-ов и TLS-политик запуска.

use super::server_loop;
use rustls::ServerConfig;
use std::{io, path::Path, sync::Arc};

/// Политика TLS применяется до чтения протокольного greeting-а.
#[derive(Clone)]
pub(crate) enum TlsMode {
    Plain,
    Implicit(Arc<ServerConfig>),
    StartTls(Arc<ServerConfig>),
}

/// Запускает plaintext SMTP и IMAP для локального peer-теста без паролей.
pub fn serve(smtp_address: &str, imap_address: &str, root: impl AsRef<Path>) -> io::Result<()> {
    serve_two_protocols(smtp_address, imap_address, root, TlsMode::Plain)
}

/// Запускает implicit-TLS SMTP submission и IMAP с одним TLS material.
pub fn serve_tls(
    smtp_address: &str,
    imap_address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    serve_two_protocols(
        smtp_address,
        imap_address,
        root,
        TlsMode::Implicit(tls_config),
    )
}

/// Запускает SMTP и IMAP с доступным SMTP STARTTLS upgrade.
pub fn serve_starttls(
    smtp_address: &str,
    imap_address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    serve_two_protocols(
        smtp_address,
        imap_address,
        root,
        TlsMode::StartTls(tls_config),
    )
}

/// Собирает два listener-а общего SMTP/IMAP локального режима.
fn serve_two_protocols(
    smtp_address: &str,
    imap_address: &str,
    root: impl AsRef<Path>,
    tls_mode: TlsMode,
) -> io::Result<()> {
    server_loop::serve(
        Some(smtp_address),
        Some(imap_address),
        None,
        None,
        None,
        root,
        tls_mode,
    )
}

/// Запускает POP3 с обязательным STLS перед передачей password.
pub fn serve_pop3_starttls(
    address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    server_loop::serve(
        None,
        None,
        Some(address),
        None,
        None,
        root,
        TlsMode::StartTls(tls_config),
    )
}

/// Запускает отдельный ManageSieve STARTTLS listener с общим Store owner.
pub fn serve_managesieve_starttls(
    address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    server_loop::serve(
        None,
        None,
        None,
        Some(address),
        None,
        root,
        TlsMode::StartTls(tls_config),
    )
}

/// Запускает POP3 с implicit TLS, не выдавая greeting до handshake.
pub fn serve_pop3_tls(
    address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    server_loop::serve(
        None,
        None,
        Some(address),
        None,
        None,
        root,
        TlsMode::Implicit(tls_config),
    )
}

/// Запускает все входящие протоколы в одном reactor-е и Store owner-е.
pub fn serve_all_mail_starttls(
    smtp_address: &str,
    imap_address: &str,
    pop3_address: &str,
    sieve_address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    serve_all_mail(
        smtp_address,
        imap_address,
        pop3_address,
        sieve_address,
        root,
        TlsMode::StartTls(tls_config),
    )
}

/// Запускает все протоколы с TLS handshake до первого протокольного байта.
pub fn serve_all_mail_implicit_tls(
    smtp_address: &str,
    imap_address: &str,
    pop3_address: &str,
    sieve_address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    serve_all_mail(
        smtp_address,
        imap_address,
        pop3_address,
        sieve_address,
        root,
        TlsMode::Implicit(tls_config),
    )
}

/// Передаёт четыре listener-а одному владельцу TLS/session lifecycle.
fn serve_all_mail(
    smtp_address: &str,
    imap_address: &str,
    pop3_address: &str,
    sieve_address: &str,
    root: impl AsRef<Path>,
    tls_mode: TlsMode,
) -> io::Result<()> {
    server_loop::serve(
        Some(smtp_address),
        Some(imap_address),
        Some(pop3_address),
        Some(sieve_address),
        None,
        root,
        tls_mode,
    )
}
