//! Исходящий SMTP поверх TLS с явной проверкой локально заданного CA и SNI.
//!
//! Это standalone transport для route, где policy требует implicit TLS. Он не
//! разделяет plaintext pool: TLS identity является отдельным ключом ресурса.
use crate::outbound::{self, DeliveryOutcome};
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, StreamOwned,
    pki_types::{CertificateDer, ServerName},
};
use std::{
    io::{self, BufReader, Write},
    net::TcpStream,
    sync::Arc,
    time::Duration,
};

/// Выполняет одну implicit-TLS SMTP transaction, проверяя CA и server name.
pub fn deliver(
    address: &str,
    server_name: &str,
    ca_der: &[u8],
    sender: &str,
    recipient: &str,
    message: &[u8],
) -> io::Result<DeliveryOutcome> {
    let config = client_config(ca_der)?;
    let name = ServerName::try_from(server_name.to_owned())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid TLS server name"))?;
    let stream = TcpStream::connect(address)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let connection = ClientConnection::new(config, name).map_err(io::Error::other)?;
    let mut reader = BufReader::new(StreamOwned::new(connection, stream));
    expect_positive(&mut reader)?;
    command(&mut reader, "EHLO litterkeep.test\r\n")?;
    expect_positive(&mut reader)?;
    command(&mut reader, &format!("MAIL FROM:<{sender}>\r\n"))?;
    if let Some(outcome) = outbound::expect(&mut reader)? {
        return Ok(outcome);
    }
    command(&mut reader, &format!("RCPT TO:<{recipient}>\r\n"))?;
    if let Some(outcome) = outbound::expect(&mut reader)? {
        return Ok(outcome);
    }
    command(&mut reader, "DATA\r\n")?;
    if let Some(outcome) = outbound::expect(&mut reader)? {
        return Ok(outcome);
    }
    let stream = reader.get_mut();
    stream.write_all(message)?;
    stream.write_all(b"\r\n.\r\n")?;
    stream.flush()?;
    match outbound::expect(&mut reader)? {
        None => Ok(DeliveryOutcome::Delivered),
        Some(outcome) => Ok(outcome),
    }
}

fn client_config(ca_der: &[u8]) -> io::Result<Arc<ClientConfig>> {
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from(ca_der.to_vec()))
        .map_err(io::Error::other)?;
    Ok(Arc::new(
        ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    ))
}

fn command(
    reader: &mut BufReader<StreamOwned<ClientConnection, TcpStream>>,
    wire: &str,
) -> io::Result<()> {
    let stream = reader.get_mut();
    stream.write_all(wire.as_bytes())?;
    stream.flush()
}

fn expect_positive(
    reader: &mut BufReader<StreamOwned<ClientConnection, TcpStream>>,
) -> io::Result<()> {
    match outbound::expect(reader)? {
        None => Ok(()),
        Some(DeliveryOutcome::Deferred) => Err(io::Error::other("TLS SMTP peer deferred")),
        Some(DeliveryOutcome::PermanentFailure) => Err(io::Error::other("TLS SMTP peer rejected")),
        Some(DeliveryOutcome::Delivered) => Err(io::Error::other("unexpected SMTP outcome")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{CertificateParams, CertifiedIssuer, KeyPair};
    use rustls::{
        ServerConfig, ServerConnection,
        pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer},
    };
    use std::{io::BufRead, net::TcpListener, thread};

    #[test]
    fn trusted_tls_peer_receives_smtp_message() {
        let (ca, certificate, key) = certificate_chain();
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let peer = thread::spawn(move || serve_peer(listener, certificate, key));
        let outcome = deliver(
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
    fn malformed_ca_is_rejected_before_connect() {
        assert!(client_config(b"not a DER certificate").is_err());
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

    fn serve_peer(listener: TcpListener, certificate: Vec<u8>, key: Vec<u8>) {
        let (stream, _) = listener.accept().expect("accept");
        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(certificate)],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key)),
            )
            .expect("server config");
        let connection = ServerConnection::new(Arc::new(config)).expect("connection");
        let mut reader = BufReader::new(StreamOwned::new(connection, stream));
        write_line(&mut reader, b"220 peer ready\r\n");
        read_line(&mut reader, "EHLO");
        write_line(&mut reader, b"250 peer\r\n");
        read_line(&mut reader, "MAIL FROM:");
        write_line(&mut reader, b"250 sender\r\n");
        read_line(&mut reader, "RCPT TO:");
        write_line(&mut reader, b"250 recipient\r\n");
        read_line(&mut reader, "DATA");
        write_line(&mut reader, b"354 data\r\n");
        read_message(&mut reader);
        write_line(&mut reader, b"250 accepted\r\n");
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
}
