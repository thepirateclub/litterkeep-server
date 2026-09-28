//! Независимый POP3 SCRAM-SHA-256-PLUS peer для уже запущенного TLS listener.

#[path = "pop3_scram_sha256_plus_peer.rs"]
mod peer;

use peer::{
    cancellation_preserves_user, connect, expect_prefix, read_multiline, rejects_invalid_proof,
    rejects_wrong_channel_binding, required, write_line,
};

/// Запускается явно против независимого implicit-TLS POP3 listener-а.
#[test]
#[ignore = "requires POP3_TLS_ADDRESS, POP3_CA_PEM and POP3_TEST_PASSWORD"]
fn verifies_scram_sha256_plus_with_rfc9266_exporter() {
    let address = required("POP3_TLS_ADDRESS");
    let ca_pem = std::fs::read_to_string(required("POP3_CA_PEM")).expect("read CA PEM");
    let ca = decode_pem_certificate(&ca_pem);
    let password = required("POP3_TEST_PASSWORD");
    provision_sha256_verifier(&address, &ca, &password);
    let mut peer = connect(&address, &ca);
    expect_prefix(&mut peer, "+OK", "implicit TLS greeting");
    write_line(&mut peer, b"CAPA\r\n");
    let capabilities = read_multiline(&mut peer);
    assert!(
        capabilities
            .iter()
            .any(|line| line == b"SASL PLAIN SCRAM-SHA-1 SCRAM-SHA-256 SCRAM-SHA-256-PLUS\r\n"),
        "TLS CAPA must advertise exporter-bound SCRAM"
    );
    cancellation_preserves_user(&address, &ca, &password);
    let exporter = peer
        .get_ref()
        .conn
        .export_keying_material([0; 32], b"EXPORTER-Channel-Binding", Some(&[]))
        .expect("RFC 9266 TLS exporter");
    rejects_wrong_channel_binding(&address, &ca, &password);
    rejects_invalid_proof(&address, &ca);
    peer::authenticate_plus(&mut peer, &password, &exporter);
    write_line(&mut peer, b"STAT\r\n");
    expect_prefix(&mut peer, "+OK", "STAT after PLUS authentication");
    write_line(&mut peer, b"QUIT\r\n");
    expect_prefix(&mut peer, "+OK", "QUIT after PLUS authentication");
}

/// Создаёт SHA-256 verifier штатным успешным входом старой acceptance-учётки.
fn provision_sha256_verifier(address: &str, ca: &[u8], password: &str) {
    let mut peer = connect(address, ca);
    expect_prefix(&mut peer, "+OK", "provision greeting");
    write_line(&mut peer, b"USER bob\r\n");
    expect_prefix(&mut peer, "+OK", "provision USER");
    write_line(&mut peer, format!("PASS {password}\r\n").as_bytes());
    expect_prefix(&mut peer, "+OK", "provision PASS");
    write_line(&mut peer, b"QUIT\r\n");
    expect_prefix(&mut peer, "+OK", "provision QUIT");
}

/// Извлекает DER октеты локального CA из одного PEM certificate block.
fn decode_pem_certificate(pem: &str) -> Vec<u8> {
    let body = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<String>();
    litterkeep_server::mime::decode_base64(body.as_bytes()).expect("decode CA certificate")
}
