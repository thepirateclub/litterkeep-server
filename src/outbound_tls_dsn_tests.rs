//! Проверки TLS forwarding strategy для сгруппированных DSN origins.

use crate::{
    outbound::DeliveryOutcome,
    outbound_tls_pool::ConnectionPool,
    queue::DsnDeliveryMetadata,
    smtp::{
        MailDsn, NotifyCondition, NotifyPolicy, OriginalAddressType, OriginalRecipient,
        RecipientDsn,
    },
};
use rcgen::{CertificateParams, CertifiedIssuer, KeyPair};
use rustls::{ServerConfig, ServerConnection, pki_types::PrivateKeyDer};
use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
};

/// TLS peer принимает письмо без неоднозначных downstream DSN параметров.
#[test]
fn grouped_tls_forwarding_uses_local_relay_notice_strategy() {
    let (ca, certificate, key) = certificate_chain();
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || serve_grouped_peer(listener, certificate, key));
    let metadata = DsnDeliveryMetadata {
        mail: MailDsn {
            return_content: Some(crate::smtp::ReturnContent::Headers),
            envelope_id: Some(b"trace id".to_vec()),
        },
        recipient: None,
        origin_recipients: vec![
            Some(recipient(NotifyCondition::Success, "first@origin.test")),
            Some(recipient(NotifyCondition::Failure, "second@origin.test")),
        ],
    };
    let mut pool = ConnectionPool::new(address.to_string(), "peer.test", ca, 1).expect("TLS pool");

    let attempt = pool
        .deliver_detailed_with_dsn(
            "alice@origin.test",
            "bob@forward.test",
            b"Subject: TLS grouped\r\n\r\nbody",
            Some(&metadata),
        )
        .expect("delivery");

    assert_eq!(attempt.outcome, DeliveryOutcome::Relayed);
    pool.shutdown().expect("QUIT");
    peer.join().expect("TLS peer");
}

/// Создаёт локальный CA и серверный сертификат для SNI `peer.test`.
fn certificate_chain() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let ca_key = KeyPair::generate().expect("CA key");
    let ca_params = CertificateParams::new(Vec::<String>::new()).expect("CA params");
    let issuer = CertifiedIssuer::self_signed(ca_params, ca_key).expect("CA cert");
    let ca = issuer.der().to_vec();
    let peer_key = KeyPair::generate().expect("peer key");
    let mut peer_params = CertificateParams::new(vec!["peer.test".into()]).expect("peer params");
    peer_params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "peer.test");
    let peer = peer_params
        .signed_by(&peer_key, &issuer)
        .expect("peer cert");
    (ca, peer.der().to_vec(), peer_key.serialize_der())
}

/// Независимый peer проверяет каждый этап TLS SMTP transaction.
fn serve_grouped_peer(listener: TcpListener, certificate: Vec<u8>, key: Vec<u8>) {
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![rustls::pki_types::CertificateDer::from(certificate)],
            PrivateKeyDer::Pkcs8(key.into()),
        )
        .expect("TLS config");
    let (stream, _) = listener.accept().expect("accept");
    let connection = ServerConnection::new(Arc::new(config)).expect("TLS connection");
    let mut reader = BufReader::new(rustls::StreamOwned::new(connection, stream));
    let writer = reader.get_mut();
    writer.write_all(b"220 peer ready\r\n").expect("greeting");
    expect_line(&mut reader, "EHLO litterkeep.test\r\n");
    reader
        .get_mut()
        .write_all(b"250-peer.test\r\n250-DSN\r\n250 SIZE 100000\r\n")
        .expect("EHLO reply");
    expect_line(&mut reader, "MAIL FROM:<alice@origin.test> SIZE=30\r\n");
    reader.get_mut().write_all(b"250 sender\r\n").expect("MAIL");
    expect_line(&mut reader, "RCPT TO:<bob@forward.test>\r\n");
    reader
        .get_mut()
        .write_all(b"250 recipient\r\n")
        .expect("RCPT");
    expect_line(&mut reader, "DATA\r\n");
    reader.get_mut().write_all(b"354 data\r\n").expect("DATA");
    read_message(&mut reader);
    reader
        .get_mut()
        .write_all(b"250 accepted\r\n")
        .expect("accepted");
    expect_line(&mut reader, "QUIT\r\n");
    reader
        .get_mut()
        .write_all(b"221 closing\r\n")
        .expect("QUIT");
}

/// Проверяет ожидаемую команду и тем самым обнаруживает лишние DSN поля.
fn expect_line(
    reader: &mut BufReader<rustls::StreamOwned<ServerConnection, TcpStream>>,
    expected: &str,
) {
    let mut line = String::new();
    reader.read_line(&mut line).expect("SMTP line");
    assert_eq!(line, expected);
}

/// Считывает DATA до terminator без интерпретации содержимого.
fn read_message(reader: &mut BufReader<rustls::StreamOwned<ServerConnection, TcpStream>>) {
    let mut line = String::new();
    loop {
        line.clear();
        reader.read_line(&mut line).expect("DATA line");
        if line == ".\r\n" {
            return;
        }
    }
}

/// Создаёт typed recipient policy без сырых сравнений поля.
fn recipient(condition: NotifyCondition, address: &str) -> RecipientDsn {
    RecipientDsn {
        notify: Some(NotifyPolicy::Conditions([condition].into_iter().collect())),
        original_recipient: Some(OriginalRecipient {
            address_type: OriginalAddressType::Rfc822,
            address: address.as_bytes().to_vec(),
        }),
    }
}
