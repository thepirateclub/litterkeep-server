//! Изолированная проверка command timeout после implicit-TLS greeting.

use crate::{outbound::SmtpTimeouts, outbound_tls};
use rcgen::{CertificateParams, CertifiedIssuer, KeyPair};
use rustls::{
    ServerConfig, ServerConnection, StreamOwned,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

/// Доказывает, что TLS EHLO использует command, а не истёкший greeting deadline.
#[test]
fn implicit_tls_ehlo_uses_command_timeout_after_greeting() {
    let (ca, certificate, key) = certificate_chain();
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || serve_silent_ehlo(listener, certificate, key));
    let limits =
        SmtpTimeouts::greeting_and_command(Duration::from_secs(2), Duration::from_millis(100));
    let started = Instant::now();
    let result = outbound_tls::Connection::connect_for_timeout_test(
        &address.to_string(),
        "peer.test",
        &ca,
        limits,
    );
    let elapsed = started.elapsed();
    assert!(result.is_err(), "silent EHLO must time out");
    assert!(
        elapsed < Duration::from_secs(1),
        "EHLO exceeded its command deadline"
    );
    peer.join().expect("TLS peer");
}

/// Создаёт независимые корневой сертификат и серверный сертификат теста.
fn certificate_chain() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let ca_key = KeyPair::generate().expect("CA key");
    let ca =
        CertifiedIssuer::self_signed(CertificateParams::default(), ca_key).expect("CA certificate");
    let server_key = KeyPair::generate().expect("server key");
    let params = CertificateParams::new(vec!["peer.test".into()]).expect("server names");
    let server = params
        .signed_by(&server_key, &ca)
        .expect("server certificate");
    (
        ca.der().to_vec(),
        server.der().to_vec(),
        server_key.serialize_der(),
    )
}

/// Отвечает на greeting и EHLO, затем держит TLS канал до закрытия клиента.
fn serve_silent_ehlo(listener: TcpListener, certificate: Vec<u8>, key: Vec<u8>) {
    let config = server_config(certificate, key);
    let (stream, _) = listener.accept().expect("accept");
    let connection = ServerConnection::new(config).expect("TLS connection");
    let mut reader = BufReader::new(StreamOwned::new(connection, stream));
    reader
        .get_mut()
        .write_all(b"220 peer ready\r\n")
        .expect("greeting");
    reader.get_mut().flush().expect("flush greeting");
    let mut ehlo = String::new();
    reader.read_line(&mut ehlo).expect("EHLO line");
    assert!(ehlo.starts_with("EHLO "), "unexpected command: {ehlo}");
    let mut unexpected = String::new();
    assert!(matches!(reader.read_line(&mut unexpected), Ok(0) | Err(_)));
}

/// Конфигурирует TLS listener с единственным тестовым leaf certificate.
fn server_config(certificate: Vec<u8>, key: Vec<u8>) -> Arc<ServerConfig> {
    Arc::new(
        ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(certificate)],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key)),
            )
            .expect("server config"),
    )
}
