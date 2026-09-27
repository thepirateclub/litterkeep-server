//! Создание plaintext SMTP connection до готового ESMTP/HELO состояния.

use super::{
    Connection, EhloCapabilities, EhloNegotiation, ReplyExpectation, TransportLimits,
    connect_bounded_addr, expect_success, negotiate_ehlo,
};
use std::{
    io::{self, BufReader},
    net::SocketAddr,
};

/// Устанавливает TCP, применяет фазовые deadlines и выполняет greeting/EHLO.
pub(super) fn connect_addr_with_limits(
    address: SocketAddr,
    limits: TransportLimits,
) -> io::Result<Connection> {
    let stream = connect_bounded_addr(address, limits)?;
    let mut connection = Connection {
        reader: BufReader::new(stream.try_clone()?),
        writer: stream,
        capabilities: EhloCapabilities::default(),
        close_after_response: false,
        timeouts: limits.timeouts,
    };
    expect_success(&mut connection.reader, ReplyExpectation::Greeting)?;
    set_command_deadline(&connection)?;
    connection.command("EHLO litterkeep.test\r\n")?;
    match negotiate_ehlo(&mut connection.reader)? {
        EhloNegotiation::Extended(capabilities) => connection.capabilities = capabilities,
        EhloNegotiation::Basic => {
            connection.command("HELO litterkeep.test\r\n")?;
            expect_success(&mut connection.reader, ReplyExpectation::Helo)?;
        }
    }
    Ok(connection)
}

/// Переключает greeting read timeout на нормативный общий command deadline.
fn set_command_deadline(connection: &Connection) -> io::Result<()> {
    connection
        .reader
        .get_ref()
        .set_read_timeout(Some(connection.timeouts.command()))
}
