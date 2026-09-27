//! TLS wire-проверки исходящего SMTP transport-а.

use crate::outbound::DeliveryOutcome;
use crate::outbound_tls;
use rcgen::{CertificateParams, CertifiedIssuer, KeyPair};
use rustls::StreamOwned;
use rustls::{
    ServerConfig, ServerConnection,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
};

#[test]
fn trusted_tls_peer_receives_smtp_message() {
    let (ca, certificate, key) = certificate_chain();
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || {
        serve_peer(
            listener,
            certificate,
            key,
            1,
            b"250 peer\r\n",
            "MAIL FROM:",
            true,
        )
    });
    let outcome = outbound_tls::deliver(
        &address.to_string(),
        "peer.test",
        &ca,
        "alice@test",
        "remote@test",
        b"Subject: TLS\r\n\r\nbody",
    )
    .expect("delivery");
    assert_eq!(outcome, DeliveryOutcome::Delivered);
    peer.join().expect("peer");
}

#[test]
fn implicit_tls_falls_back_to_helo_without_using_unadvertised_extensions() {
    let (ca, certificate, key) = certificate_chain();
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || serve_helo_fallback_peer(listener, certificate, key));
    assert_eq!(
        outbound_tls::deliver(
            &address.to_string(),
            "peer.test",
            &ca,
            "alice@test",
            "bob@test",
            b"body",
        )
        .expect("TLS delivery after HELO fallback"),
        DeliveryOutcome::Delivered
    );
    peer.join().expect("peer");
}

#[test]
fn malformed_ca_is_rejected_before_connect() {
    assert!(outbound_tls::client_config(b"not a DER certificate").is_err());
}

#[test]
fn implicit_tls_uses_ehlo_8bitmime_for_utf8_body() {
    let (ca, certificate, key) = certificate_chain();
    let message = ("Content-Type: text/plain; charset=UTF-8\r\n\
Content-Transfer-Encoding: 8bit\r\n\r\nПривет")
        .as_bytes()
        .to_vec();
    let size = message.len().saturating_add(2);
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || {
        serve_peer(
            listener,
            certificate,
            key,
            1,
            b"250-peer.test\r\n250-SIZE 1024\r\n250 8BITMIME\r\n",
            &format!("MAIL FROM:<alice@test> SIZE={size} BODY=8BITMIME"),
            true,
        )
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
        .expect("TLS delivery"),
        DeliveryOutcome::Delivered
    );
    peer.join().expect("peer");
}

#[test]
fn tls_pool_reuses_only_its_bound_identity_context() {
    let (ca, certificate, key) = certificate_chain();
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || {
        serve_peer(
            listener,
            certificate,
            key,
            2,
            b"250 peer\r\n",
            "MAIL FROM:",
            false,
        )
    });
    let mut pool =
        crate::outbound_tls_pool::ConnectionPool::new(address.to_string(), "peer.test", ca, 1)
            .expect("pool");
    for recipient in ["first@test", "second@test"] {
        assert_eq!(
            pool.deliver("alice@test", recipient, b"Subject: TLS\r\n\r\nbody")
                .expect("delivery"),
            DeliveryOutcome::Delivered
        );
    }
    assert_eq!(pool.idle_count(), 1);
    peer.join().expect("peer");
}

fn certificate_chain() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let ca_params = CertificateParams::default();
    let ca_key = KeyPair::generate().expect("CA key");
    let ca = CertifiedIssuer::self_signed(ca_params, ca_key).expect("CA");
    let server_params = CertificateParams::new(vec!["peer.test".into()]).expect("names");
    let key = KeyPair::generate().expect("server key");
    let certificate = server_params.signed_by(&key, &ca).expect("certificate");
    (
        ca.der().to_vec(),
        certificate.der().to_vec(),
        key.serialize_der(),
    )
}

fn serve_peer(
    listener: TcpListener,
    certificate: Vec<u8>,
    key: Vec<u8>,
    transactions: usize,
    ehlo_reply: &[u8],
    mail_prefix: &str,
    quit_after_delivery: bool,
) {
    let config = server_config(certificate, key);
    let mut reader = accept_tls_peer(&listener, &config);
    write_line(&mut reader, b"220 peer ready\r\n");
    read_line(&mut reader, "EHLO");
    write_line(&mut reader, ehlo_reply);
    for _ in 0..transactions {
        read_line(&mut reader, mail_prefix);
        write_line(&mut reader, b"250 sender\r\n");
        serve_transaction(&mut reader);
    }
    if quit_after_delivery {
        read_line_exact(&mut reader, "QUIT\r\n");
        write_line(&mut reader, b"221 closing\r\n");
    }
}

fn serve_helo_fallback_peer(listener: TcpListener, certificate: Vec<u8>, key: Vec<u8>) {
    let config = server_config(certificate, key);
    let mut reader = accept_tls_peer(&listener, &config);
    write_line(&mut reader, b"220 peer ready\r\n");
    read_line(&mut reader, "EHLO");
    write_line(&mut reader, b"500 command not recognized\r\n");
    read_line_exact(&mut reader, "HELO litterkeep.test\r\n");
    write_line(&mut reader, b"250 hello\r\n");
    read_line_exact(&mut reader, "MAIL FROM:<alice@test>\r\n");
    write_line(&mut reader, b"250 sender\r\n");
    serve_transaction(&mut reader);
    read_line_exact(&mut reader, "QUIT\r\n");
    write_line(&mut reader, b"221 closing\r\n");
}

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

fn accept_tls_peer(
    listener: &TcpListener,
    config: &Arc<ServerConfig>,
) -> BufReader<StreamOwned<ServerConnection, TcpStream>> {
    let (stream, _) = listener.accept().expect("accept");
    let connection = ServerConnection::new(Arc::clone(config)).expect("connection");
    BufReader::new(StreamOwned::new(connection, stream))
}

fn serve_transaction(reader: &mut BufReader<StreamOwned<ServerConnection, TcpStream>>) {
    read_line(reader, "RCPT TO:");
    write_line(reader, b"250 recipient\r\n");
    read_line(reader, "DATA");
    write_line(reader, b"354 data\r\n");
    read_message(reader);
    write_line(reader, b"250 accepted\r\n");
}

fn write_line(reader: &mut BufReader<StreamOwned<ServerConnection, TcpStream>>, wire: &[u8]) {
    reader.get_mut().write_all(wire).expect("write");
    reader.get_mut().flush().expect("flush");
}

fn read_line(reader: &mut BufReader<StreamOwned<ServerConnection, TcpStream>>, prefix: &str) {
    let mut line = String::new();
    reader.read_line(&mut line).expect("read");
    assert!(line.starts_with(prefix), "unexpected command: {line}");
}

fn read_line_exact(
    reader: &mut BufReader<StreamOwned<ServerConnection, TcpStream>>,
    expected: &str,
) {
    let mut line = String::new();
    reader.read_line(&mut line).expect("command");
    assert_eq!(line, expected);
}

fn read_message(reader: &mut BufReader<StreamOwned<ServerConnection, TcpStream>>) {
    let mut line = String::new();
    loop {
        line.clear();
        reader.read_line(&mut line).expect("message");
        if line == ".\r\n" {
            return;
        }
    }
}
