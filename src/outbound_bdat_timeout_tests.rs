//! Socket-level проверка отдельных ответных timeout для исходящего BDAT.

use super::{Connection, DeliveryOutcome, TransportLimits, bdat::binary_chunks};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

#[derive(Clone, Copy)]
enum DelayedReply {
    Intermediate,
    Last,
}

/// Промежуточный chunk должен прерываться коротким command timeout.
#[test]
fn intermediate_bdat_reply_uses_command_timeout_on_socket() {
    let mut message = binary_message();
    message.extend(std::iter::repeat_n(0xA5, 70_000));
    let error = run_peer(message, DelayedReply::Intermediate).expect_err("short timeout fires");
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    ));
}

/// BDAT LAST получает длинный DATA-termination timeout и успевает ответить.
#[test]
fn last_bdat_reply_uses_data_termination_timeout_on_socket() {
    let message = binary_message();
    let outcome = run_peer(message, DelayedReply::Last).expect("long timeout remains open");

    assert!(matches!(outcome, DeliveryOutcome::Delivered));
}

fn run_peer(message: Vec<u8>, delayed_reply: DelayedReply) -> std::io::Result<DeliveryOutcome> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("SMTP test listener");
    let address = listener.local_addr().expect("listener address");
    let peer_message = message.clone();
    let peer = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept outbound connection");
        serve_delayed_peer(stream, &peer_message, delayed_reply);
    });
    let limits = TransportLimits::bdat_phases(
        Duration::from_secs(1),
        Duration::from_millis(100),
        Duration::from_millis(700),
    );
    let outcome = {
        let mut connection = Connection::connect_with_limits(&address.to_string(), limits)
            .expect("connect test peer");
        connection.deliver("alice@test", "remote@test", &message)
    };
    peer.join().expect("peer phase assertions");
    outcome
}

fn serve_delayed_peer(mut stream: TcpStream, message: &[u8], delayed_reply: DelayedReply) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("peer read timeout");
    let mut reader = BufReader::new(stream.try_clone().expect("clone peer stream"));
    stream.write_all(b"220 peer ready\r\n").expect("greeting");
    expect_line(&mut reader, "EHLO litterkeep.test\r\n");
    stream
        .write_all(b"250-peer.test\r\n250-BINARYMIME\r\n250-CHUNKING\r\n250 SIZE 1000000\r\n")
        .expect("capabilities");
    expect_line(
        &mut reader,
        &format!(
            "MAIL FROM:<alice@test> SIZE={} BODY=BINARYMIME\r\n",
            message.len()
        ),
    );
    stream.write_all(b"250 sender\r\n").expect("MAIL reply");
    expect_line(&mut reader, "RCPT TO:<remote@test>\r\n");
    stream.write_all(b"250 recipient\r\n").expect("RCPT reply");
    serve_chunks(&mut stream, &mut reader, message, delayed_reply);
}

fn serve_chunks(
    stream: &mut TcpStream,
    reader: &mut BufReader<TcpStream>,
    message: &[u8],
    delayed_reply: DelayedReply,
) {
    for chunk in binary_chunks(message) {
        let marker = match chunk.last {
            true => " LAST",
            false => "",
        };
        expect_line(reader, &format!("BDAT {}{}\r\n", chunk.bytes.len(), marker));
        let mut payload = vec![0; chunk.bytes.len()];
        reader.read_exact(&mut payload).expect("BDAT payload");
        assert_eq!(payload, chunk.bytes);
        match (delayed_reply, chunk.last) {
            (DelayedReply::Intermediate, false) => {
                thread::sleep(Duration::from_millis(250));
                let mut extra = String::new();
                assert_eq!(
                    reader
                        .read_line(&mut extra)
                        .expect("timed-out client closed"),
                    0
                );
                return;
            }
            (DelayedReply::Last, true) => thread::sleep(Duration::from_millis(250)),
            _ => {}
        }
        stream
            .write_all(b"250 chunk accepted\r\n")
            .expect("BDAT reply");
    }
}

fn binary_message() -> Vec<u8> {
    b"Content-Type: application/octet-stream\r\nContent-Transfer-Encoding: binary\r\n\r\npayload\0\xff"
        .to_vec()
}

fn expect_line(reader: &mut BufReader<TcpStream>, expected: &str) {
    let mut actual = String::new();
    reader.read_line(&mut actual).expect("SMTP command line");
    assert_eq!(actual, expected);
}
