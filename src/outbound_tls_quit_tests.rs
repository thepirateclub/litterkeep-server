//! Проверка TLS QUIT reply timeout и сохранения результата доставки.

use crate::{outbound::DeliveryOutcome, outbound::SmtpTimeouts, outbound_tls};
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
    time::Duration,
};

/// Именует TLS peer, который завершает доставку, но не отвечает на QUIT.
#[test]
fn implicit_tls_quit_wait_is_bounded_after_accepted_message() {
    let (ca, certificate, key) = certificate_chain();
    let listener = TcpListener::bind("127.0.0.1:0").expect("TLS listener");
    let address = listener.local_addr().expect("listener address");
    let peer = thread::spawn(move || serve_silent_quit(listener, certificate, key));
    let limits = SmtpTimeouts::uniform(Duration::from_millis(100));
    let (delivery, quit) = {
        let mut connection = outbound_tls::Connection::connect_for_timeout_test(
            &address.to_string(),
            "peer.test",
            &ca,
            limits,
        )
        .expect("TLS SMTP session");
        let delivery = connection
            .deliver("alice@test", "remote@test", b"Subject: TLS\r\n\r\nbody")
            .expect("DATA delivery");
        (delivery, connection.quit())
    };

    assert!(matches!(delivery, DeliveryOutcome::Delivered));
    let error = quit.expect_err("peer omits QUIT reply");
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    ));
    peer.join().expect("TLS peer saw bounded client close");
}

/// Принимает письмо и QUIT, но намеренно удерживает TLS канал без ответа 221.
fn serve_silent_quit(listener: TcpListener, certificate: Vec<u8>, key: Vec<u8>) {
    let config = server_config(certificate, key);
    let (socket, _) = listener.accept().expect("accept TLS client");
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("peer read timeout");
    let connection = ServerConnection::new(config).expect("TLS server connection");
    let mut reader = BufReader::new(StreamOwned::new(connection, socket));
    write_line(&mut reader, b"220 peer ready\r\n");
    expect_line(&mut reader, b"EHLO litterkeep.test\r\n");
    write_line(&mut reader, b"250 peer.test\r\n");
    expect_line(&mut reader, b"MAIL FROM:<alice@test>\r\n");
    write_line(&mut reader, b"250 sender\r\n");
    expect_line(&mut reader, b"RCPT TO:<remote@test>\r\n");
    write_line(&mut reader, b"250 recipient\r\n");
    expect_line(&mut reader, b"DATA\r\n");
    write_line(&mut reader, b"354 send data\r\n");
    read_message(&mut reader);
    write_line(&mut reader, b"250 accepted\r\n");
    expect_line(&mut reader, b"QUIT\r\n");
    thread::sleep(Duration::from_millis(250));
    let mut extra = Vec::new();
    assert!(matches!(
        reader.read_until(b'\n', &mut extra),
        Ok(0) | Err(_)
    ));
}

/// Создаёт тестовый CA и подписанный сертификат для SNI имени peer.test.
fn certificate_chain() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let ca_key = KeyPair::generate().expect("CA key");
    let ca = CertifiedIssuer::self_signed(CertificateParams::default(), ca_key).expect("CA cert");
    let server_key = KeyPair::generate().expect("server key");
    let params = CertificateParams::new(vec!["peer.test".into()]).expect("server name");
    let server = params.signed_by(&server_key, &ca).expect("server cert");
    (
        ca.der().to_vec(),
        server.der().to_vec(),
        server_key.serialize_der(),
    )
}

/// Строит rustls listener без клиентской аутентификации.
fn server_config(certificate: Vec<u8>, key: Vec<u8>) -> Arc<ServerConfig> {
    Arc::new(
        ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(certificate)],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key)),
            )
            .expect("TLS peer configuration"),
    )
}

/// Считывает ровно одну SMTP-команду вместе с CRLF и сравнивает байты.
fn expect_line(reader: &mut BufReader<StreamOwned<ServerConnection, TcpStream>>, expected: &[u8]) {
    let mut actual = Vec::new();
    reader.read_until(b'\n', &mut actual).expect("SMTP command");
    assert_eq!(actual, expected);
}

/// Читает DATA до отдельной строки-terminator, не декодируя тело в UTF-8.
fn read_message(reader: &mut BufReader<StreamOwned<ServerConnection, TcpStream>>) {
    loop {
        let mut line = Vec::new();
        reader.read_until(b'\n', &mut line).expect("SMTP DATA line");
        if line.as_slice() == b".\r\n" {
            return;
        }
    }
}

/// Записывает полный SMTP-ответ и сбрасывает TLS writer для peer.
fn write_line(reader: &mut BufReader<StreamOwned<ServerConnection, TcpStream>>, response: &[u8]) {
    reader.get_mut().write_all(response).expect("SMTP reply");
    reader.get_mut().flush().expect("flush SMTP reply");
}
