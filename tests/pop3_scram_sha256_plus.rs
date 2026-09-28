//! Независимый POP3 SCRAM-SHA-256-PLUS peer для уже запущенного TLS listener.

#[path = "pop3_scram_sha256_plus_peer.rs"]
mod peer;
#[path = "support/pop3_scram_sha256_plus_starttls_peer.rs"]
mod starttls_peer;

use peer::{
    cancellation_preserves_user, connect, decode_pem_certificate, expect_prefix, read_multiline,
    rejects_invalid_proof, rejects_wrong_channel_binding, required, write_line,
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
    write_line(&mut peer, b"AUTH SCRAM-SHA-256-PLUS\r\n");
    expect_prefix(&mut peer, "-ERR", "AUTH after successful authentication");
    write_line(&mut peer, b"STAT\r\n");
    expect_prefix(&mut peer, "+OK", "STAT after PLUS authentication");
    write_line(&mut peer, b"QUIT\r\n");
    expect_prefix(&mut peer, "+OK", "QUIT after PLUS authentication");
}

/// Проверяет отдельный POP3 путь STLS и появление exporter-bound механизма.
#[test]
#[ignore = "requires POP3_STARTTLS_ADDRESS, POP3_CA_PEM and POP3_TEST_PASSWORD"]
fn verifies_scram_sha256_plus_after_stls() {
    let address = peer::required("POP3_STARTTLS_ADDRESS");
    let pem = std::fs::read_to_string(peer::required("POP3_CA_PEM")).expect("read CA PEM");
    let ca = peer::decode_pem_certificate(&pem);
    let password = peer::required("POP3_TEST_PASSWORD");
    starttls_peer::provision_sha256_verifier(&address, &ca, &password);
    starttls_peer::authenticate_plus(&address, &ca, &password);
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
