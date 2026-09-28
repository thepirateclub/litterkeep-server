//! Независимый TLS upgrade peer для POP3 SCRAM-SHA-256-PLUS.

use super::peer::{self, Peer};
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use std::{
    io::{BufReader, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

/// Создаёт SHA-256 verifier штатным USER/PASS через STLS listener.
pub fn provision_sha256_verifier(address: &str, ca: &[u8], password: &str) {
    let mut peer = upgrade(address, ca);
    peer::write_line(&mut peer, b"USER bob\r\n");
    peer::expect_prefix(&mut peer, "+OK", "STARTTLS verifier USER");
    peer::write_line(&mut peer, format!("PASS {password}\r\n").as_bytes());
    peer::expect_prefix(&mut peer, "+OK", "STARTTLS verifier PASS");
    peer::write_line(&mut peer, b"QUIT\r\n");
    peer::expect_prefix(&mut peer, "+OK", "STARTTLS verifier QUIT");
}

/// Проверяет TLS CAPA, exporter и полный PLUS login после STLS.
pub fn authenticate_plus(address: &str, ca: &[u8], password: &str) {
    let mut connection = connect(address);
    expect_raw_prefix(&mut connection, "+OK", "STARTTLS greeting");
    connection
        .write_all(b"CAPA\r\n")
        .expect("write plaintext CAPA");
    let clear = read_raw_multiline(&mut connection);
    assert!(clear.iter().any(|line| line == b"STLS\r\n"));
    assert!(!clear.iter().any(|line| line.starts_with(b"SASL ")));
    connection.write_all(b"STLS\r\n").expect("write STLS");
    expect_raw_prefix(&mut connection, "+OK", "STLS response");
    let mut peer = tls_peer(connection, ca);
    peer::write_line(&mut peer, b"CAPA\r\n");
    let encrypted = peer::read_multiline(&mut peer);
    assert!(
        encrypted
            .iter()
            .any(|line| { line == b"SASL PLAIN SCRAM-SHA-1 SCRAM-SHA-256 SCRAM-SHA-256-PLUS\r\n" })
    );
    let exporter = peer
        .get_ref()
        .conn
        .export_keying_material([0; 32], b"EXPORTER-Channel-Binding", Some(&[]))
        .expect("RFC 9266 exporter after STLS");
    peer::authenticate_plus(&mut peer, password, &exporter);
    peer::write_line(&mut peer, b"STAT\r\n");
    peer::expect_prefix(&mut peer, "+OK", "STAT after STARTTLS PLUS");
    peer::write_line(&mut peer, b"QUIT\r\n");
    peer::expect_prefix(&mut peer, "+OK", "QUIT after STARTTLS PLUS");
}

/// Выполняет STARTTLS upgrade для предварительного создания verifier sidecar.
fn upgrade(address: &str, ca: &[u8]) -> Peer {
    let mut connection = connect(address);
    expect_raw_prefix(&mut connection, "+OK", "verifier greeting");
    connection
        .write_all(b"STLS\r\n")
        .expect("write verifier STLS");
    expect_raw_prefix(&mut connection, "+OK", "verifier STLS response");
    tls_peer(connection, ca)
}

/// Создаёт проверяющий TLS peer и требует согласования TLS 1.3.
fn tls_peer(connection: TcpStream, ca: &[u8]) -> Peer {
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from(ca.to_vec()))
        .expect("add STARTTLS CA");
    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = ServerName::try_from("localhost".to_owned()).expect("TLS server name");
    let client = ClientConnection::new(Arc::new(config), name).expect("TLS client");
    let mut peer = BufReader::new(StreamOwned::new(client, connection));
    while peer.get_ref().conn.is_handshaking() {
        let stream = peer.get_mut();
        stream
            .conn
            .complete_io(&mut stream.sock)
            .expect("complete STARTTLS handshake");
    }
    assert_eq!(
        peer.get_ref().conn.protocol_version(),
        Some(rustls::ProtocolVersion::TLSv1_3)
    );
    peer
}

/// Открывает TCP с таймаутами, сохраняя незашифрованный socket без prefetch.
fn connect(address: &str) -> TcpStream {
    let target = address
        .to_socket_addrs()
        .expect("resolve POP3 STARTTLS address")
        .next()
        .expect("at least one STARTTLS address");
    let stream = TcpStream::connect_timeout(&target, Duration::from_secs(10))
        .expect("connect POP3 STARTTLS");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("set STARTTLS read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .expect("set STARTTLS write timeout");
    stream
}

/// Читает plaintext CAPA целиком до POP3 dot terminator.
fn read_raw_multiline(stream: &mut TcpStream) -> Vec<Vec<u8>> {
    expect_raw_prefix(stream, "+OK", "plaintext CAPA status");
    let mut lines = Vec::new();
    loop {
        let line = read_raw_line(stream, "plaintext CAPA line");
        if line == b".\r\n" {
            return lines;
        }
        lines.push(line);
    }
}

/// Проверяет status line без BufReader, чтобы не прочитать TLS bytes заранее.
fn expect_raw_prefix(stream: &mut TcpStream, prefix: &str, operation: &str) {
    let line = read_raw_line(stream, operation);
    assert!(line.starts_with(prefix.as_bytes()), "{operation}: {line:?}");
}

/// Читает ровно одну plaintext строку байт за байтом до CRLF.
fn read_raw_line(stream: &mut TcpStream, operation: &str) -> Vec<u8> {
    let mut line = Vec::new();
    loop {
        let mut octet = [0_u8; 1];
        stream
            .read_exact(&mut octet)
            .unwrap_or_else(|error| panic!("{operation}: {error}"));
        line.push(octet[0]);
        if line.ends_with(b"\r\n") {
            return line;
        }
    }
}
