"""Проверяет POP3 command octet limit и сохранение синхронизации после отказа."""

import os
import socket
import ssl


HOST = os.environ.get("LITTERKEEP_POP3_HOST", "127.0.0.1")
PORT = int(os.environ.get("LITTERKEEP_POP3_PORT", "1110"))


def read_line(stream):
    line = stream.readline()
    if not line.endswith(b"\r\n"):
        raise AssertionError(f"expected POP3 CRLF line, received {line!r}")
    return line


def expect_line(stream, prefix, context):
    line = read_line(stream)
    if not line.startswith(prefix):
        raise AssertionError(f"{context}: expected {prefix!r}, got {line!r}")
    return line


def send_line(connection, stream, wire, prefix, context):
    connection.sendall(wire)
    return expect_line(stream, prefix, context)


def main():
    tls_context = ssl.create_default_context()
    tls_context.check_hostname = False
    tls_context.verify_mode = ssl.CERT_NONE
    connection = socket.create_connection((HOST, PORT), timeout=10)
    stream = connection.makefile("rb")
    try:
        expect_line(stream, b"+OK", "POP3 greeting")
        send_line(connection, stream, b"STLS\r\n", b"+OK", "STLS")
        stream.close()
        connection = tls_context.wrap_socket(connection, server_hostname="localhost")
        stream = connection.makefile("rb")
        send_line(connection, stream, b"CAPA\r\n", b"+OK", "CAPA after TLS")
        while read_line(stream) != b".\r\n":
            continue

        maximum = b"USER " + b"u" * 248 + b"\r\n"
        if len(maximum) != 255:
            raise AssertionError(f"test fixture boundary is {len(maximum)} octets")
        send_line(connection, stream, maximum, b"+OK", "255-octet command")

        over_limit = b"USER " + b"u" * 249 + b"\r\n"
        if len(over_limit) != 256:
            raise AssertionError(f"test fixture overflow is {len(over_limit)} octets")
        send_line(connection, stream, over_limit, b"-ERR", "256-octet command")
        send_line(connection, stream, b"USER bob\r\n", b"+OK", "command after overflow")
        send_line(
            connection,
            stream,
            f"PASS {os.environ['LITTERKEEP_BOB_PASSWORD']}\r\n".encode("ascii"),
            b"+OK",
            "PASS after overflow",
        )
        send_line(connection, stream, b"STAT\r\n", b"+OK", "transaction after auth")
        send_line(connection, stream, b"QUIT\r\n", b"+OK", "QUIT")
    finally:
        stream.close()
        connection.close()

    print("PASS pop3-command-length: 255 accepted, 256 rejected, next command remains synchronized")


if __name__ == "__main__":
    main()
