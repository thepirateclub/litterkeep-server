//! Транспорт и клиентские вычисления для независимого POP3 acceptance peer.

use rustls::{
    ClientConfig, ClientConnection, RootCertStore, StreamOwned,
    pki_types::{CertificateDer, ServerName},
};
use std::{
    env,
    io::{BufRead, BufReader, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

pub type Peer = BufReader<StreamOwned<ClientConnection, TcpStream>>;

/// Извлекает DER октеты локального CA из одного PEM certificate block.
pub fn decode_pem_certificate(pem: &str) -> Vec<u8> {
    let body = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<String>();
    litterkeep_server::mime::decode_base64(body.as_bytes()).expect("decode CA certificate")
}

/// Отменяет незавершённый PLUS и подтверждает сохранение выбранного USER.
pub fn cancellation_preserves_user(address: &str, ca: &[u8], password: &str) {
    let mut peer = connect(address, ca);
    expect_prefix(&mut peer, "+OK", "cancellation greeting");
    write_line(&mut peer, b"USER bob\r\n");
    expect_prefix(&mut peer, "+OK", "USER before SASL");
    write_line(&mut peer, b"AUTH SCRAM-SHA-256-PLUS\r\n");
    expect_prefix(&mut peer, "+ ", "PLUS client-first prompt");
    write_line(&mut peer, b"*\r\n");
    expect_prefix(&mut peer, "-ERR", "PLUS cancellation");
    write_line(&mut peer, format!("PASS {password}\r\n").as_bytes());
    expect_prefix(&mut peer, "+OK", "PASS after cancellation");
    write_line(&mut peer, b"QUIT\r\n");
    expect_prefix(&mut peer, "+OK", "QUIT after cancellation recovery");
}

/// Отвергает transcript с чужим exporter и сохраняет POP3 сессию.
pub fn rejects_wrong_channel_binding(address: &str, ca: &[u8], password: &str) {
    let mut peer = connect(address, ca);
    expect_prefix(&mut peer, "+OK", "wrong-binding greeting");
    let server_first = begin_exchange(&mut peer);
    let payload = final_payload(password, &server_first, [0x11; 32]);
    write_line(
        &mut peer,
        format!("{}\r\n", base64(payload.as_bytes())).as_bytes(),
    );
    expect_prefix(&mut peer, "-ERR", "wrong channel binding");
    write_line(&mut peer, b"USER bob\r\n");
    expect_prefix(
        &mut peer,
        "+OK",
        "authorization recovery after wrong binding",
    );
}

/// Отвергает неверный client proof при syntactically верном channel binding.
pub fn rejects_invalid_proof(address: &str, ca: &[u8]) {
    let mut peer = connect(address, ca);
    expect_prefix(&mut peer, "+OK", "invalid-proof greeting");
    let server_first = begin_exchange(&mut peer);
    let exporter = peer
        .get_ref()
        .conn
        .export_keying_material([0; 32], b"EXPORTER-Channel-Binding", Some(&[]))
        .expect("RFC 9266 TLS exporter");
    let payload = final_payload("wrong-password", &server_first, exporter);
    write_line(
        &mut peer,
        format!("{}\r\n", base64(payload.as_bytes())).as_bytes(),
    );
    expect_prefix(&mut peer, "+ ", "server-final for invalid proof");
    write_line(&mut peer, b"\r\n");
    expect_prefix(&mut peer, "-ERR", "invalid client proof");
    write_line(&mut peer, b"USER bob\r\n");
    expect_prefix(
        &mut peer,
        "+OK",
        "authorization recovery after invalid proof",
    );
}

/// Начинает PLUS exchange и возвращает декодированную server-first строку.
pub fn begin_exchange(peer: &mut Peer) -> String {
    let first = "p=tls-exporter,,n=bob,r=negative-pop3-client";
    write_line(
        peer,
        format!("AUTH SCRAM-SHA-256-PLUS {}\r\n", base64(first.as_bytes())).as_bytes(),
    );
    decode_challenge(&read_line(peer, "negative server-first"))
}

/// Строит корректный proof поверх заведомо выбранного binding.
fn final_payload(password: &str, server_first: &str, binding: [u8; 32]) -> String {
    let nonce = attribute(server_first, "r");
    let salt = litterkeep_server::mime::decode_base64(attribute(server_first, "s").as_bytes())
        .expect("server salt Base64");
    let iterations = attribute(server_first, "i")
        .parse::<u32>()
        .expect("iteration count");
    let mut gs2 = b"p=tls-exporter,,".to_vec();
    gs2.extend_from_slice(&binding);
    let without_proof = format!("c={},r={nonce}", base64(&gs2));
    let transcript = format!("n=bob,r=negative-pop3-client,{server_first},{without_proof}");
    let (proof, _) = proof(password.as_bytes(), &salt, iterations, &transcript);
    format!("{without_proof},p={}", base64(&proof))
}

/// Подтверждает server-final для правильного TLS exporter и пароля.
pub fn authenticate_plus(peer: &mut Peer, password: &str, exporter: &[u8]) {
    let client_first = "p=tls-exporter,,n=bob,r=independent-pop3-client";
    write_line(
        peer,
        format!(
            "AUTH SCRAM-SHA-256-PLUS {}\r\n",
            base64(client_first.as_bytes())
        )
        .as_bytes(),
    );
    let server_first = decode_challenge(&read_line(peer, "SCRAM server-first"));
    let nonce = attribute(&server_first, "r");
    let salt = litterkeep_server::mime::decode_base64(attribute(&server_first, "s").as_bytes())
        .expect("server salt Base64");
    let iterations = attribute(&server_first, "i")
        .parse::<u32>()
        .expect("server iteration count");
    let client_bare = "n=bob,r=independent-pop3-client";
    let mut binding = b"p=tls-exporter,,".to_vec();
    binding.extend_from_slice(exporter);
    let without_proof = format!("c={},r={nonce}", base64(&binding));
    let transcript = format!("{client_bare},{server_first},{without_proof}");
    let (proof, server_signature) = proof(password.as_bytes(), &salt, iterations, &transcript);
    let final_message = format!("{without_proof},p={}", base64(&proof));
    write_line(
        peer,
        format!("{}\r\n", base64(final_message.as_bytes())).as_bytes(),
    );
    let server_final = decode_challenge(&read_line(peer, "SCRAM server-final"));
    assert_eq!(
        attribute(&server_final, "v"),
        base64(&server_signature),
        "server-final signature must match the PLUS transcript"
    );
    write_line(peer, b"\r\n");
    expect_prefix(peer, "+OK", "empty SASL acknowledgement and maildrop");
}

/// Проверяет локальный CA и требует TLS 1.3 для RFC 9266.
pub fn connect(address: &str, ca_der: &[u8]) -> Peer {
    let socket_address = address
        .to_socket_addrs()
        .expect("resolve POP3 test address")
        .next()
        .expect("at least one socket address");
    let socket = TcpStream::connect_timeout(&socket_address, Duration::from_secs(10))
        .expect("connect to POP3 listener");
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("set POP3 read timeout");
    socket
        .set_write_timeout(Some(Duration::from_secs(10)))
        .expect("set POP3 write timeout");
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from(ca_der.to_vec()))
        .expect("add acceptance CA");
    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = ServerName::try_from("localhost".to_owned()).expect("TLS server name");
    let connection = ClientConnection::new(Arc::new(config), name).expect("TLS client");
    let mut peer = BufReader::new(StreamOwned::new(connection, socket));
    while peer.get_ref().conn.is_handshaking() {
        let stream = peer.get_mut();
        stream
            .conn
            .complete_io(&mut stream.sock)
            .expect("complete POP3 TLS handshake");
    }
    assert_eq!(
        peer.get_ref().conn.protocol_version(),
        Some(rustls::ProtocolVersion::TLSv1_3),
        "RFC 9266 PLUS vector requires TLS 1.3"
    );
    peer
}

/// Читает полный POP3 multiline CAPA ответ.
pub fn read_multiline(peer: &mut Peer) -> Vec<Vec<u8>> {
    expect_prefix(peer, "+OK", "CAPA status");
    let mut lines = Vec::new();
    loop {
        let line = read_line(peer, "CAPA line");
        if line == b".\r\n" {
            return lines;
        }
        lines.push(line);
    }
}

/// Декодирует POP3 SASL continuation в UTF-8 SCRAM message.
pub fn decode_challenge(line: &[u8]) -> String {
    let encoded = std::str::from_utf8(line)
        .expect("ASCII SASL challenge")
        .strip_prefix("+ ")
        .and_then(|value| value.strip_suffix("\r\n"))
        .expect("POP3 SASL continuation");
    String::from_utf8(litterkeep_server::mime::decode_base64(encoded.as_bytes()).expect("Base64"))
        .expect("UTF-8 SCRAM message")
}

/// Вычисляет client proof и server signature SCRAM-SHA-256.
fn proof(password: &[u8], salt: &[u8], iterations: u32, transcript: &str) -> (Vec<u8>, [u8; 32]) {
    let salted = litterkeep_server::sasl::pbkdf2_sha256::derive(password, salt, iterations)
        .expect("SCRAM salted password");
    let client_key = litterkeep_server::sasl::hmac_sha256::authenticate(&salted, b"Client Key");
    let stored_key = litterkeep_server::sasl::sha256::digest(&client_key);
    let client_signature =
        litterkeep_server::sasl::hmac_sha256::authenticate(&stored_key, transcript.as_bytes());
    let proof = client_key
        .iter()
        .zip(client_signature)
        .map(|(key, signature)| *key ^ signature)
        .collect();
    let server_key = litterkeep_server::sasl::hmac_sha256::authenticate(&salted, b"Server Key");
    let signature =
        litterkeep_server::sasl::hmac_sha256::authenticate(&server_key, transcript.as_bytes());
    (proof, signature)
}

/// Находит требуемый атрибут в comma-separated SCRAM сообщении.
fn attribute<'a>(message: &'a str, name: &str) -> &'a str {
    message
        .split(',')
        .find_map(|field| field.strip_prefix(&format!("{name}=")))
        .expect("SCRAM attribute")
}

/// Кодирует стандартный Base64 для POP3 SASL payload.
pub fn base64(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(input.len().div_ceil(3).saturating_mul(4));
    for chunk in input.chunks(3) {
        let first = u32::from(chunk.first().copied().unwrap_or_default());
        let second = u32::from(chunk.get(1).copied().unwrap_or_default());
        let third = u32::from(chunk.get(2).copied().unwrap_or_default());
        let bits = (first << 16) | (second << 8) | third;
        for shift in [18_u32, 12, 6, 0] {
            let alphabet_index = usize::try_from((bits >> shift) & 63).unwrap_or_default();
            output.push(char::from(
                ALPHABET.get(alphabet_index).copied().unwrap_or(b'A'),
            ));
        }
        match chunk.len() {
            1 => output.replace_range(output.len().saturating_sub(2).., "=="),
            2 => output.replace_range(output.len().saturating_sub(1).., "="),
            _ => {}
        }
    }
    output
}

/// Отправляет команду целиком и сбрасывает TLS plaintext.
pub fn write_line(peer: &mut Peer, line: &[u8]) {
    peer.get_mut().write_all(line).expect("write POP3 command");
    peer.get_mut().flush().expect("flush POP3 command");
}

/// Проверяет POP3 status prefix и включает весь ответ в диагностику.
pub fn expect_prefix(peer: &mut Peer, prefix: &str, operation: &str) {
    let line = read_line(peer, operation);
    assert!(line.starts_with(prefix.as_bytes()), "{operation}: {line:?}");
}

/// Читает единственную POP3 CRLF строку с контекстом операции.
pub fn read_line(peer: &mut Peer, operation: &str) -> Vec<u8> {
    let mut line = Vec::new();
    peer.read_until(b'\n', &mut line)
        .unwrap_or_else(|error| panic!("{operation}: {error}"));
    assert!(
        line.ends_with(b"\r\n"),
        "{operation}: malformed line {line:?}"
    );
    line
}

/// Требует переменную окружения для явного socket acceptance.
pub fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("set {name} for the explicit wire acceptance"))
}
