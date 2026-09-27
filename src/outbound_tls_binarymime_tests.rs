//! TLS wire acceptance для исходящего RFC 3030 BINARYMIME/BDAT транспорта.
use crate::{outbound::DeliveryOutcome, outbound_tls};
use rcgen::{CertificateParams, CertifiedIssuer, KeyPair};
use rustls::{
    ServerConfig, ServerConnection, StreamOwned,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
    time::Duration,
};

/// TLS peer подтверждает MAIL, границы каждого BDAT и точные payload octets.
#[test]
fn implicit_tls_binarymime_sends_unmodified_chunks() {
    let (ca, certificate, key) = certificate_chain();
    let listener = TcpListener::bind("127.0.0.1:0").expect("TLS SMTP listener");
    let address = listener.local_addr().expect("listener address");
    let mut message = b"Content-Type: application/octet-stream\r\nContent-Transfer-Encoding: binary\r\n\r\n\0\xff\r\n\n\r".to_vec();
    message.extend(std::iter::repeat_n(0x5A, 70_000));
    let peer_message = message.clone();
    let peer = thread::spawn(move || {
        verify_binary_peer(listener, certificate, key, &peer_message);
    });

    assert_eq!(
        outbound_tls::deliver(
            &address.to_string(),
            "peer.test",
            &ca,
            "alice@test",
            "remote@test",
            &message,
        )
        .expect("TLS BINARYMIME delivery"),
        DeliveryOutcome::Delivered
    );
    peer.join().expect("TLS peer assertions");
}

/// Выполняет TLS greeting, capability exchange и exact-chunk прием.
fn verify_binary_peer(listener: TcpListener, certificate: Vec<u8>, key: Vec<u8>, message: &[u8]) {
    let config = server_config(certificate, key);
    let mut reader = accept_tls_peer(&listener, &config);
    reader
        .get_ref()
        .sock
        .set_read_timeout(Some(Duration::from_secs(4)))
        .expect("TLS peer read timeout");
    write_line(&mut reader, b"220 peer ready\r\n");
    expect_line(&mut reader, "EHLO litterkeep.test\r\n");
    write_line(
        &mut reader,
        b"250-peer.test\r\n250-BINARYMIME\r\n250-CHUNKING\r\n250 SIZE 1000000\r\n",
    );
    let mail = format!(
        "MAIL FROM:<alice@test> SIZE={} BODY=BINARYMIME\r\n",
        message.len()
    );
    expect_line(&mut reader, &mail);
    write_line(&mut reader, b"250 sender\r\n");
    expect_line(&mut reader, "RCPT TO:<remote@test>\r\n");
    write_line(&mut reader, b"250 recipient\r\n");
    verify_chunks(&mut reader, message);
    expect_line(&mut reader, "QUIT\r\n");
    write_line(&mut reader, b"221 closing\r\n");
}

/// Сверяет каждую команду, принимает только её exact octet count и отвечает 250.
fn verify_chunks(reader: &mut BufReader<StreamOwned<ServerConnection, TcpStream>>, message: &[u8]) {
    for chunk in crate::outbound::bdat::binary_chunks(message) {
        let marker = match chunk.last {
            true => " LAST",
            false => "",
        };
        expect_line(reader, &format!("BDAT {}{}\r\n", chunk.bytes.len(), marker));
        let mut received = vec![0; chunk.bytes.len()];
        reader
            .read_exact(&mut received)
            .expect("read exact TLS BDAT");
        assert_eq!(received, chunk.bytes);
        write_line(reader, b"250 chunk accepted\r\n");
    }
}

/// Создаёт независимые CA и server certificate для локального TLS peer.
fn certificate_chain() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let ca_key = KeyPair::generate().expect("CA key");
    let ca = CertifiedIssuer::self_signed(CertificateParams::default(), ca_key).expect("CA");
    let params = CertificateParams::new(vec!["peer.test".into()]).expect("server SAN");
    let key = KeyPair::generate().expect("server key");
    let certificate = params.signed_by(&key, &ca).expect("server certificate");
    (
        ca.der().to_vec(),
        certificate.der().to_vec(),
        key.serialize_der(),
    )
}

/// Создаёт серверный rustls config с одним локальным сертификатом.
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

/// Устанавливает TLS session и оборачивает её reader-ом для SMTP команд.
fn accept_tls_peer(
    listener: &TcpListener,
    config: &Arc<ServerConfig>,
) -> BufReader<StreamOwned<ServerConnection, TcpStream>> {
    let (socket, _) = listener.accept().expect("accept TLS connection");
    let connection = ServerConnection::new(Arc::clone(config)).expect("TLS server connection");
    BufReader::new(StreamOwned::new(connection, socket))
}

/// Читает одну SMTP строку и сравнивает весь command вместе с CRLF.
fn expect_line(reader: &mut BufReader<StreamOwned<ServerConnection, TcpStream>>, expected: &str) {
    let mut actual = String::new();
    reader.read_line(&mut actual).expect("read SMTP command");
    assert_eq!(actual, expected);
}

/// Передаёт reply через уже согласованный TLS stream.
fn write_line(reader: &mut BufReader<StreamOwned<ServerConnection, TcpStream>>, response: &[u8]) {
    reader
        .get_mut()
        .write_all(response)
        .expect("write SMTP reply");
    reader.get_mut().flush().expect("flush SMTP reply");
}
