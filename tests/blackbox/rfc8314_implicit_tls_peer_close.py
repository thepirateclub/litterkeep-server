"""Проверяет peer-инициированный TLS shutdown для implicit SMTP и IMAP."""

import argparse
from enum import Enum
import socket
import ssl


class Protocol(Enum):
    """Endpoint-ы протоколов, для которых RFC 8314 задаёт closure guidance."""

    SMTP = "smtp"
    IMAP = "imap"


def read_line(stream, protocol):
    """Требует полную CRLF-завершённую строку до TLS close_notify."""
    line = stream.readline()
    if not line.endswith(b"\r\n"):
        raise AssertionError(f"{protocol.value}: malformed line {line!r}")
    return line


def exchange_command(connection, stream, protocol):
    """Получает обычный protocol reply, не посылая команду закрытия."""
    match protocol:
        case Protocol.SMTP:
            connection.sendall(b"NOOP\r\n")
            reply = read_line(stream, protocol)
            if not reply.startswith(b"250 "):
                raise AssertionError(f"SMTP NOOP failed: {reply!r}")
        case Protocol.IMAP:
            connection.sendall(b"A1 NOOP\r\n")
            reply = read_line(stream, protocol)
            if not reply.startswith(b"A1 OK "):
                raise AssertionError(f"IMAP NOOP failed: {reply!r}")


def check_endpoint(host, port, protocol, expected_greeting):
    """Проверяет TLS greeting, protocol NOOP и ответный close_notify."""
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    transport = socket.create_connection((host, port), timeout=10)
    connection = context.wrap_socket(transport, server_hostname="localhost")
    connection.settimeout(10)
    stream = connection.makefile("rwb", buffering=0)
    try:
        greeting = read_line(stream, protocol)
        if not greeting.startswith(expected_greeting):
            raise AssertionError(f"{protocol.value}: unexpected greeting {greeting!r}")
        exchange_command(connection, stream, protocol)
    finally:
        stream.close()
    raw = connection.unwrap()
    raw.close()


def main():
    """Запускает closure acceptance на обоих implicit-TLS listener-ах."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, default=1465)
    parser.add_argument("--imap-port", type=int, default=1993)
    options = parser.parse_args()
    check_endpoint(options.host, options.smtp_port, Protocol.SMTP, b"220 ")
    check_endpoint(options.host, options.imap_port, Protocol.IMAP, b"* OK ")
    print("PASS RFC 8314: implicit SMTP and IMAP answer peer close_notify")


if __name__ == "__main__":
    main()
