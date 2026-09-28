//! Проверки TLS exporter между независимыми rustls endpoint-ами.

use crate::tls_transport::ServerTls;
use rcgen::generate_simple_self_signed;
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, ServerConfig,
    pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer, ServerName},
    version::TLS13,
};
use std::sync::Arc;

/// TLS 1.3 exporter совпадает на обеих сторонах для RFC 9266 параметров.
#[test]
fn tls13_channel_binding_matches_client_exporter() {
    let certificate = generate_simple_self_signed(vec!["localhost".to_owned()])
        .expect("self-signed TLS test certificate");
    let certificate_der = certificate.cert.der().clone();
    let server_config = ServerConfig::builder_with_protocol_versions(&[&TLS13])
        .with_no_client_auth()
        .with_single_cert(
            vec![certificate_der.clone()],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
                certificate.signing_key.serialize_der(),
            )),
        )
        .expect("TLS 1.3 server config");
    let mut roots = RootCertStore::empty();
    roots.add(certificate_der).expect("test root certificate");
    let client_config = ClientConfig::builder_with_protocol_versions(&[&TLS13])
        .with_root_certificates(roots)
        .with_no_client_auth();
    let server = ServerTls::new(Arc::new(server_config)).expect("server endpoint");
    let name = ServerName::try_from("localhost").expect("server name");
    let client = ClientConnection::new(Arc::new(client_config), name).expect("client endpoint");
    let (server_binding, client_binding) = complete_handshake(server, client);
    assert_eq!(server_binding, Some(client_binding));
}

/// Переносит TLS records между endpoint-ами без runtime или socket wrappers.
fn complete_handshake(
    mut server: ServerTls,
    mut client: ClientConnection,
) -> (Option<[u8; 32]>, [u8; 32]) {
    for _ in 0..8 {
        transfer_client_flight(&mut client, &mut server);
        transfer_server_flight(&mut server, &mut client);
        if !client.is_handshaking() && !server.is_handshaking() {
            let client_binding = client
                .export_keying_material([0; 32], b"EXPORTER-Channel-Binding", Some(&[]))
                .expect("client TLS exporter");
            return (server.tls_exporter_channel_binding(), client_binding);
        }
    }
    panic!("TLS handshake did not complete within the bounded flight count");
}

/// Обрабатывает один необязательный client-to-server flight.
fn transfer_client_flight(client: &mut ClientConnection, server: &mut ServerTls) {
    let mut flight = Vec::new();
    client.write_tls(&mut flight).expect("client TLS output");
    if flight.is_empty() {
        return;
    }
    server.receive(&flight).expect("server TLS input");
}

/// Обрабатывает один необязательный server-to-client flight.
fn transfer_server_flight(server: &mut ServerTls, client: &mut ClientConnection) {
    let flight = server.take_encrypted().expect("server TLS output");
    if flight.is_empty() {
        return;
    }
    let mut input = flight.as_slice();
    client.read_tls(&mut input).expect("client TLS input");
    client.process_new_packets().expect("client TLS record");
}
