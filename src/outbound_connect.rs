//! Создание plaintext SMTP connection до готового ESMTP/HELO состояния.

use super::{
    Connection, EhloCapabilities, EhloNegotiation, ReplyExpectation, TransportLimits,
    expect_success, negotiate_ehlo,
};
use std::{
    io::{self, BufReader},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

const GENERIC_IO_TIMEOUT: Duration = Duration::from_secs(10);

/// Создаёт только числовой TCP route с конечными connect/read/write deadline.
pub(crate) fn connect_bounded(address: &str) -> io::Result<TcpStream> {
    let stream = TcpStream::connect_timeout(&parse_route(address)?, Duration::from_secs(5))?;
    stream.set_read_timeout(Some(GENERIC_IO_TIMEOUT))?;
    stream.set_write_timeout(Some(GENERIC_IO_TIMEOUT))?;
    Ok(stream)
}

pub(crate) fn connect_bounded_with_timeouts(
    address: &str,
    timeouts: super::SmtpTimeouts,
) -> io::Result<TcpStream> {
    connect_bounded_addr(
        parse_route(address)?,
        super::TransportLimits {
            connect: Duration::from_secs(5),
            timeouts,
        },
    )
}

pub(crate) fn parse_route(address: &str) -> io::Result<SocketAddr> {
    address.parse::<SocketAddr>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "outbound route must be an explicit socket address",
        )
    })
}

fn connect_bounded_addr(
    address: SocketAddr,
    limits: super::TransportLimits,
) -> io::Result<TcpStream> {
    let stream = TcpStream::connect_timeout(&address, limits.connect)?;
    stream.set_read_timeout(Some(limits.timeouts.greeting()))?;
    stream.set_write_timeout(Some(limits.timeouts.command()))?;
    Ok(stream)
}

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
        last_diagnostic: None,
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
