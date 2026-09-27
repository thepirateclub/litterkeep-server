"""Проверяет TLS close_notify по RFC 8314 для POP3 implicit TLS."""

import base64
import os
import socket
import ssl


HOST = os.environ.get("LITTERKEEP_HOST", "127.0.0.1")
PORT = int(os.environ.get("LITTERKEEP_POP3_PORT", "995"))


def read_line(stream):
    line = stream.readline()
    if not line.endswith(b"\r\n"):
        raise AssertionError(f"invalid POP3 line: {line!r}")
    return line


def command(connection, stream, value, expected):
    connection.sendall(value + b"\r\n")
    reply = read_line(stream)
    if not reply.startswith(expected):
        raise AssertionError(f"{value!r}: expected {expected!r}, got {reply!r}")


def authenticate(connection, stream, password):
    """Захватывает POP3 maildrop lease через TLS-only SASL PLAIN."""
    credentials = base64.b64encode(b"\x00bob\x00" + password)
    command(connection, stream, b"AUTH PLAIN " + credentials, b"+OK")


def peer_initiated_close(context, password):
    """Проверяет alert и освобождение maildrop lease без POP3 QUIT."""
    transport = socket.create_connection((HOST, PORT), timeout=10)
    tls = context.wrap_socket(transport, server_hostname="localhost")
    tls.settimeout(10)
    stream = tls.makefile("rb")
    try:
        if not read_line(stream).startswith(b"+OK"):
            raise AssertionError("implicit-TLS greeting was not positive")
        authenticate(tls, stream, password)
    finally:
        stream.close()
    raw = tls.unwrap()
    raw.close()


def main():
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    password = os.environ["LITTERKEEP_BOB_PASSWORD"].encode("ascii")
    peer_initiated_close(context, password)
    transport = socket.create_connection((HOST, PORT), timeout=10)
    tls = context.wrap_socket(transport, server_hostname="localhost")
    stream = tls.makefile("rb")
    try:
        if not read_line(stream).startswith(b"+OK"):
            raise AssertionError("implicit-TLS greeting was not positive")
        authenticate(tls, stream, password)
        command(tls, stream, b"QUIT", b"+OK")
        stream.close()
        raw = tls.unwrap()
        if raw.recv(1) != b"":
            raise AssertionError("server sent application data after TLS close alert")
        raw.close()
    finally:
        stream.close()
        tls.close()
    print("PASS POP3 implicit TLS: peer alert is answered; QUIT reply precedes close_notify")


if __name__ == "__main__":
    main()
