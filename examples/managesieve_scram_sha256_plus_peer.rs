#[rustfmt::skip]
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned, pki_types::{CertificateDer, ServerName, pem::PemObject}};
#[rustfmt::skip]
use std::{env, fs, io::{self, BufRead, Write}, net::TcpStream, path::PathBuf, sync::Arc, time::Duration};
const MAX_LINE_BYTES: usize = 8192;
fn main() {
    match run() {
        Ok(()) => println!("PASS ManageSieve SCRAM-SHA-256-PLUS TLS exchange"),
        Err(error) => {
            eprintln!("SCRAM-SHA-256-PLUS failed: {error}");
            std::process::exit(1);
        }
    }
}

/// Читает адрес сервера, PEM CA и локальную acceptance-учётку.
fn run() -> io::Result<()> {
    let mut arguments = env::args().skip(1);
    let address = required_argument(&mut arguments, "server address")?;
    let ca_pem = PathBuf::from(required_argument(&mut arguments, "CA certificate PEM")?);
    let credentials = PathBuf::from(required_argument(&mut arguments, "credentials TSV")?);
    match arguments.next() {
        None => {}
        Some(_) => return Err(io::Error::other("unexpected argument")),
    }
    let password = bob_password(&credentials)?;
    let (mut stream, exporter) = connect(&address, &ca_pem)?;
    authenticate(&mut stream, &exporter, &password)
}

fn required_argument(
    arguments: &mut impl Iterator<Item = String>,
    label: &str,
) -> io::Result<String> {
    arguments
        .next()
        .ok_or_else(|| io::Error::other(format!("missing {label}")))
}

/// Загружает учётку Bob, не печатая пароль.
fn bob_password(path: &PathBuf) -> io::Result<Vec<u8>> {
    let contents = fs::read_to_string(path)?;
    for row in contents.lines() {
        let mut fields = row.splitn(2, '\t');
        if let (Some("bob"), Some(password)) = (fields.next(), fields.next()) {
            return Ok(password.as_bytes().to_vec());
        }
    }
    Err(io::Error::other("bob credential is missing"))
}

/// Проверяет TLS 1.3 handshake и извлекает RFC 9266 exporter.
fn connect(
    address: &str,
    certificate: &PathBuf,
) -> io::Result<(StreamOwned<ClientConnection, TcpStream>, [u8; 32])> {
    let mut roots = RootCertStore::empty();
    let mut certificates = CertificateDer::pem_file_iter(certificate).map_err(io::Error::other)?;
    let mut added_root = false;
    for certificate in &mut certificates {
        roots
            .add(certificate.map_err(io::Error::other)?)
            .map_err(io::Error::other)?;
        added_root = true;
    }
    require(added_root, "CA PEM contains no certificates")?;
    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let server_name = ServerName::try_from("localhost").map_err(io::Error::other)?;
    let socket = TcpStream::connect(address)?;
    socket.set_read_timeout(Some(Duration::from_secs(10)))?;
    socket.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut connection =
        ClientConnection::new(Arc::new(config), server_name).map_err(io::Error::other)?;
    while connection.is_handshaking() {
        connection.complete_io(&mut &socket)?;
    }
    if connection.protocol_version() != Some(rustls::ProtocolVersion::TLSv1_3) {
        return Err(io::Error::other("peer did not negotiate TLS 1.3"));
    }
    let mut exporter = [0; 32];
    connection
        .export_keying_material(&mut exporter, b"EXPORTER-Channel-Binding", Some(&[]))
        .map_err(io::Error::other)?;
    Ok((StreamOwned::new(connection, socket), exporter))
}

/// Выполняет PLUS обмен и сверяет подпись.
fn authenticate(
    stream: &mut StreamOwned<ClientConnection, TcpStream>,
    exporter: &[u8; 32],
    password: &[u8],
) -> io::Result<()> {
    let (greeting, terminal) = read_result(stream)?;
    require(terminal == "OK \"ready\"", "unexpected greeting terminal")?;
    require(
        greeting
            .iter()
            .any(|line| line.contains("SCRAM-SHA-256-PLUS")),
        "server did not advertise PLUS",
    )?;
    stream.write_all(b"AUTHENTICATE \"SCRAM-SHA-256-PLUS\"\r\n")?;
    require(
        read_line(stream)? == "\"\"",
        "missing SASL initial challenge",
    )?;

    let nonce = client_nonce()?;
    let client_bare = format!("n=bob,r={nonce}");
    let client_first = format!("p=tls-exporter,,{client_bare}");
    write_quoted(stream, &base64(client_first.as_bytes()))?;
    let server_first = decode_sasl_line(stream)?;
    let server_fields = attributes(&server_first);
    let server_nonce = required_field(&server_fields, "r")?;
    require(
        server_nonce.starts_with(&nonce),
        "server nonce does not extend client nonce",
    )?;
    let salt =
        litterkeep_server::mime::decode_base64(required_field(&server_fields, "s")?.as_bytes())
            .map_err(io::Error::other)?;
    let iterations = required_field(&server_fields, "i")?
        .parse::<u32>()
        .map_err(io::Error::other)?;
    let binding = channel_binding(exporter);
    let unsigned_final = format!("c={},r={server_nonce}", base64(&binding));
    let auth_message = format!("{client_bare},{server_first},{unsigned_final}");
    let (proof, expected_server) = proof_and_signature(password, &salt, iterations, &auth_message)?;
    let client_final = format!("{unsigned_final},p={}", base64(&proof));
    write_quoted(stream, &base64(client_final.as_bytes()))?;
    verify_server_signature(stream, &expected_server)
}

/// Формирует nonce средствами CSPRNG выбранного TLS provider.
fn client_nonce() -> io::Result<String> {
    let random = rustls::crypto::ring::default_provider().secure_random;
    let mut bytes = [0; 24];
    random
        .fill(&mut bytes)
        .map_err(|_| io::Error::other("cryptographic nonce generation failed"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn channel_binding(exporter: &[u8; 32]) -> Vec<u8> {
    let mut binding = b"p=tls-exporter,,".to_vec();
    binding.extend_from_slice(exporter);
    binding
}

/// Строит proof и подпись сервера.
fn proof_and_signature(
    password: &[u8],
    salt: &[u8],
    iterations: u32,
    auth_message: &str,
) -> io::Result<(Vec<u8>, [u8; 32])> {
    let salted = litterkeep_server::sasl::pbkdf2_sha256::derive(password, salt, iterations)
        .ok_or_else(|| io::Error::other("invalid verifier iteration count"))?;
    let client_key = litterkeep_server::sasl::hmac_sha256::authenticate(&salted, b"Client Key");
    let stored_key = litterkeep_server::sasl::sha256::digest(&client_key);
    let client_signature =
        litterkeep_server::sasl::hmac_sha256::authenticate(&stored_key, auth_message.as_bytes());
    let proof = client_key
        .iter()
        .zip(client_signature)
        .map(|(key, signature)| *key ^ signature)
        .collect();
    let server_key = litterkeep_server::sasl::hmac_sha256::authenticate(&salted, b"Server Key");
    let server_signature =
        litterkeep_server::sasl::hmac_sha256::authenticate(&server_key, auth_message.as_bytes());
    Ok((proof, server_signature))
}

fn verify_server_signature(
    stream: &mut StreamOwned<ClientConnection, TcpStream>,
    expected: &[u8; 32],
) -> io::Result<()> {
    let (_, terminal) = read_result(stream)?;
    require(
        terminal.starts_with("OK (SASL \""),
        "valid PLUS proof was rejected",
    )?;
    let encoded = terminal
        .strip_prefix("OK (SASL \"")
        .and_then(|value| value.split_once('\"').map(|pair| pair.0))
        .ok_or_else(|| io::Error::other("server-final is missing"))?;
    let decoded =
        litterkeep_server::mime::decode_base64(encoded.as_bytes()).map_err(io::Error::other)?;
    require(
        decoded == format!("v={}", base64(expected)).as_bytes(),
        "server signature mismatch",
    )
}

fn read_result(stream: &mut impl BufRead) -> io::Result<(Vec<String>, String)> {
    let mut lines = Vec::new();
    loop {
        let line = read_line(stream)?;
        if line.starts_with("OK") || line.starts_with("NO") || line.starts_with("BYE") {
            return Ok((lines, line));
        }
        lines.push(line);
    }
}

fn read_line(stream: &mut impl BufRead) -> io::Result<String> {
    let mut bytes = Vec::new();
    loop {
        let available = stream.fill_buf()?;
        require(!available.is_empty(), "peer closed before CRLF")?;
        let line_end = available.iter().position(|byte| *byte == b'\n');
        let length = match line_end {
            Some(position) => position.saturating_add(1),
            None => available.len(),
        };
        bytes.extend_from_slice(available.get(..length).unwrap_or_default());
        stream.consume(length);
        require(
            bytes.len() <= MAX_LINE_BYTES,
            "protocol line exceeded bound",
        )?;
        if line_end.is_some() {
            break;
        }
    }
    let content = bytes
        .strip_suffix(b"\r\n")
        .ok_or_else(|| io::Error::other("protocol line lacks CRLF"))?;
    String::from_utf8(content.to_vec()).map_err(io::Error::other)
}

fn write_quoted(stream: &mut impl Write, value: &str) -> io::Result<()> {
    stream.write_all(b"\"")?;
    stream.write_all(value.as_bytes())?;
    stream.write_all(b"\"\r\n")?;
    stream.flush()
}

fn decode_sasl_line(stream: &mut impl BufRead) -> io::Result<String> {
    let line = read_line(stream)?;
    let encoded = line
        .strip_prefix('\"')
        .and_then(|value| value.strip_suffix('\"'))
        .ok_or_else(|| io::Error::other("expected quoted SASL challenge"))?;
    String::from_utf8(
        litterkeep_server::mime::decode_base64(encoded.as_bytes()).map_err(io::Error::other)?,
    )
    .map_err(io::Error::other)
}

fn attributes(message: &str) -> Vec<(&str, &str)> {
    message
        .split(',')
        .filter_map(|field| field.split_once('='))
        .collect()
}

fn required_field<'a>(fields: &'a [(&str, &str)], name: &str) -> io::Result<&'a str> {
    fields
        .iter()
        .find_map(|(key, value)| match *key == name {
            true => Some(*value),
            false => None,
        })
        .ok_or_else(|| io::Error::other(format!("server-first omitted {name}")))
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in bytes.chunks(3) {
        let first = u32::from(chunk.first().copied().unwrap_or(0));
        let second = u32::from(chunk.get(1).copied().unwrap_or(0));
        let third = u32::from(chunk.get(2).copied().unwrap_or(0));
        let value = (first << 16) | (second << 8) | third;
        for shift in [18_u32, 12, 6, 0] {
            let offset = usize::try_from((value >> shift) & 63).unwrap_or_default();
            output.push(char::from(ALPHABET.get(offset).copied().unwrap_or(b'A')));
        }
        match chunk.len() {
            1 => {
                output.truncate(output.len().saturating_sub(2));
                output.push_str("==");
            }
            2 => {
                output.truncate(output.len().saturating_sub(1));
                output.push('=');
            }
            _ => {}
        }
    }
    output
}
fn require(condition: bool, message: &str) -> io::Result<()> {
    match condition {
        true => Ok(()),
        false => Err(io::Error::other(message.to_owned())),
    }
}
