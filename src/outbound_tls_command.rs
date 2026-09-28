//! Командные и ожидаемые SMTP ответы внутри TLS соединения.

use crate::outbound::{self, DeliveryOutcome};
use rustls::{ClientConnection, StreamOwned};
use std::{
    io::{self, BufReader, Write},
    net::TcpStream,
};

/// Отправляет одну заранее собранную TLS SMTP-команду.
pub(super) fn command(
    reader: &mut BufReader<StreamOwned<ClientConnection, TcpStream>>,
    wire: &str,
) -> io::Result<()> {
    let stream = reader.get_mut();
    stream.write_all(wire.as_bytes())?;
    stream.flush()
}

/// Преобразует неожиданный ответ TLS peer в типизированную ошибку соединения.
pub(super) fn expect_success(
    reader: &mut BufReader<StreamOwned<ClientConnection, TcpStream>>,
    expectation: outbound::ReplyExpectation,
) -> io::Result<()> {
    match outbound::expect(reader, expectation)? {
        outbound::ReplyResult::Accepted => Ok(()),
        outbound::ReplyResult::Rejected(DeliveryOutcome::Deferred) => {
            Err(io::Error::other("TLS SMTP peer deferred"))
        }
        outbound::ReplyResult::Rejected(DeliveryOutcome::PermanentFailure) => {
            Err(io::Error::other("TLS SMTP peer rejected"))
        }
        outbound::ReplyResult::Rejected(DeliveryOutcome::Delivered) => {
            Err(io::Error::other("unexpected SMTP outcome"))
        }
        outbound::ReplyResult::Closing => Err(io::Error::new(
            io::ErrorKind::ConnectionAborted,
            "TLS SMTP peer sent 421 and is closing the channel",
        )),
    }
}
