//! Проверка восстановления TLS pool после команды peer закрыть канал ответом 421.

use crate::{outbound::DeliveryOutcome, outbound_tls_pool::ConnectionPool};
use rcgen::{CertificateParams, CertifiedIssuer, KeyPair};
use rustls::{
    ServerConfig, ServerConnection, StreamOwned,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
};

/// После 421 pool не посылает RSET и открывает новое TLS соединение.
#[test]
fn tls_pool_discards_421_channel_and_reconnects_for_the_next_delivery() {
    let (ca, certificate, key) = certificate_chain();
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || serve_421_peer(listener, certificate, key));
    let mut pool = ConnectionPool::new(address.to_string(), "peer.test", ca, 1).expect("pool");
    assert_eq!(
        pool.deliver("alice@test", "first@test", b"first")
            .expect("deferred result"),
        DeliveryOutcome::Deferred
    );
    assert_eq!(pool.idle_count(), 0);
    assert_eq!(
        pool.deliver("alice@test", "second@test", b"second")
            .expect("delivery after reconnect"),
        DeliveryOutcome::Delivered
    );
    assert_eq!(pool.idle_count(), 1);
    pool.shutdown().expect("TLS pool graceful shutdown");
    peer.join().expect("peer");
}

/// На первом соединении отвечает 421, затем принимает доставку по новому TLS каналу.
fn serve_421_peer(listener: TcpListener, certificate: Vec<u8>, key: Vec<u8>) {
    let config = server_config(certificate, key);
    let mut closing = accept_tls_peer(&listener, &config);
    write_line(&mut closing, b"220 peer ready\r\n");
    read_line(&mut closing, "EHLO");
    write_line(&mut closing, b"250 peer\r\n");
    read_line(&mut closing, "MAIL FROM:");
    write_line(&mut closing, b"250 sender\r\n");
    read_line(&mut closing, "RCPT TO:");
    write_line(&mut closing, b"421 peer shutting down\r\n");
    assert_channel_closes_without_reset(&mut closing);

    let mut replacement = accept_tls_peer(&listener, &config);
    write_line(&mut replacement, b"220 peer ready\r\n");
    read_line(&mut replacement, "EHLO");
    write_line(&mut replacement, b"250 peer\r\n");
    read_line(&mut replacement, "MAIL FROM:");
    write_line(&mut replacement, b"250 sender\r\n");
    read_transaction(&mut replacement);
    read_line(&mut replacement, "QUIT");
    write_line(&mut replacement, b"221 closing\r\n");
}

/// Создаёт CA и сертификат peer.test, которым доверяет тестовый SMTP client.
fn certificate_chain() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let ca_key = KeyPair::generate().expect("CA key");
    let ca = CertifiedIssuer::self_signed(CertificateParams::default(), ca_key).expect("CA");
    let params = CertificateParams::new(vec!["peer.test".into()]).expect("server name");
    let key = KeyPair::generate().expect("server key");
    let certificate = params.signed_by(&key, &ca).expect("certificate");
    (
        ca.der().to_vec(),
        certificate.der().to_vec(),
        key.serialize_der(),
    )
}

/// Конфигурирует серверный rustls endpoint для локального wire-теста.
fn server_config(certificate: Vec<u8>, key: Vec<u8>) -> Arc<ServerConfig> {
    Arc::new(
        ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(certificate)],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key)),
            )
            .expect("TLS peer config"),
    )
}

/// Принимает TCP peer и возвращает его как буферизованный TLS поток.
fn accept_tls_peer(
    listener: &TcpListener,
    config: &Arc<ServerConfig>,
) -> BufReader<StreamOwned<ServerConnection, TcpStream>> {
    let (socket, _) = listener.accept().expect("accept TLS peer");
    let connection = ServerConnection::new(Arc::clone(config)).expect("TLS session");
    BufReader::new(StreamOwned::new(connection, socket))
}

/// Принимает обычную DATA-транзакцию после MAIL и подтверждает её ответом 250.
fn read_transaction(reader: &mut BufReader<StreamOwned<ServerConnection, TcpStream>>) {
    read_line(reader, "RCPT TO:");
    write_line(reader, b"250 recipient\r\n");
    read_line(reader, "DATA");
    write_line(reader, b"354 data\r\n");
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("DATA line");
        if line == ".\r\n" {
            break;
        }
    }
    write_line(reader, b"250 accepted\r\n");
}

/// Записывает SMTP-ответ в TLS stream и сбрасывает его до чтения следующей команды.
fn write_line(reader: &mut BufReader<StreamOwned<ServerConnection, TcpStream>>, bytes: &[u8]) {
    reader.get_mut().write_all(bytes).expect("write peer reply");
    reader.get_mut().flush().expect("flush peer reply");
}

/// Считывает SMTP-строку и проверяет ожидаемый префикс команды.
fn read_line(reader: &mut BufReader<StreamOwned<ServerConnection, TcpStream>>, prefix: &str) {
    let mut line = String::new();
    reader.read_line(&mut line).expect("read SMTP line");
    assert!(line.starts_with(prefix), "unexpected line: {line:?}");
}

/// Проверяет закрытие 421-канала без попытки отправить RSET в shutdown peer.
fn assert_channel_closes_without_reset(
    reader: &mut BufReader<StreamOwned<ServerConnection, TcpStream>>,
) {
    let mut unexpected = String::new();
    let result = reader.read_line(&mut unexpected);
    assert!(
        matches!(result, Ok(0) | Err(_)),
        "TLS client must not send RSET after 421: {unexpected}"
    );
}
