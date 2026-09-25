"""Независимо проверяет RFC 6856 UTF8 command и порядок STLS."""

import os
import smtplib
import socket
import ssl


HOST = os.environ.get("LITTERKEEP_HOST", "127.0.0.1")
PORT = int(os.environ.get("LITTERKEEP_POP3_PORT", "1110"))
SMTP_PORT = int(os.environ.get("LITTERKEEP_SMTP_PORT", "2525"))
UTF8_MARKER = "UTF8-MARKER: привет".encode("utf-8")


def read_line(stream, context):
    line = stream.readline()
    if not line.endswith(b"\r\n"):
        raise AssertionError(f"{context}: invalid POP3 line: {line!r}")
    return line


def send(connection, stream, command, expected, context):
    connection.sendall(command)
    response = read_line(stream, context)
    if not response.startswith(expected):
        raise AssertionError(f"{context}: expected {expected!r}, got {response!r}")
    return response


def capabilities(connection, stream, context):
    send(connection, stream, b"CAPA\r\n", b"+OK", f"{context} CAPA")
    values = []
    while True:
        line = read_line(stream, f"{context} capability")
        if line == b".\r\n":
            return values
        values.append(line.rstrip(b"\r\n"))


def check_plaintext_upgrade_order():
    connection = socket.create_connection((HOST, PORT), timeout=10)
    stream = connection.makefile("rb")
    try:
        read_line(stream, "plaintext greeting")
        values = capabilities(connection, stream, "plaintext")
        if b"UTF8" not in values or b"UTF8 USER" in values:
            raise AssertionError(f"unexpected plaintext UTF8 capability: {values!r}")
        send(connection, stream, b"UTF8\r\n", b"+OK", "enable UTF8 mode")
        send(connection, stream, b"STLS\r\n", b"-ERR", "STLS after UTF8")
        send(connection, stream, b"QUIT\r\n", b"+OK", "plaintext QUIT")
    finally:
        stream.close()
        connection.close()


def check_tls_utf8_mode():
    connection = socket.create_connection((HOST, PORT), timeout=10)
    stream = connection.makefile("rb")
    try:
        read_line(stream, "TLS greeting")
        send(connection, stream, b"STLS\r\n", b"+OK", "STLS before UTF8")
        stream.close()
        context = ssl.create_default_context()
        context.check_hostname = False
        context.verify_mode = ssl.CERT_NONE
        connection = context.wrap_socket(connection, server_hostname="localhost")
        stream = connection.makefile("rb")
        values = capabilities(connection, stream, "TLS authorization")
        if b"UTF8" not in values or b"UTF8 USER" in values:
            raise AssertionError(f"unexpected TLS UTF8 capability: {values!r}")
        send(connection, stream, b"UTF8\r\n", b"+OK", "TLS UTF8 command")
        send(connection, stream, b"QUIT\r\n", b"+OK", "TLS QUIT")
    finally:
        stream.close()
        connection.close()


def check_message_download_requires_utf8_mode():
    message = (
        b"From: alice@test\r\nTo: bob@test\r\nSubject: UTF8 mode test\r\n"
        b"Content-Type: text/plain; charset=utf-8\r\n"
        b"Content-Transfer-Encoding: 8bit\r\n\r\n"
        + UTF8_MARKER
        + b"\r\n"
    )
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    with smtplib.SMTP(HOST, SMTP_PORT, timeout=10) as smtp:
        smtp.starttls(context=context)
        smtp.login("alice", os.environ["LITTERKEEP_ALICE_PASSWORD"])
        refused = smtp.sendmail(
            "alice@test", ["bob@test"], message, mail_options=["BODY=8BITMIME"]
        )
        if refused:
            raise AssertionError(f"SMTP rejected the UTF-8 test message: {refused!r}")

    connection = socket.create_connection((HOST, PORT), timeout=10)
    stream = connection.makefile("rb")
    try:
        read_line(stream, "message UTF8 greeting")
        send(connection, stream, b"STLS\r\n", b"+OK", "message STLS")
        stream.close()
        connection = context.wrap_socket(connection, server_hostname="localhost")
        stream = connection.makefile("rb")
        send(connection, stream, b"USER bob\r\n", b"+OK", "message USER")
        send(
            connection,
            stream,
            f"PASS {os.environ['LITTERKEEP_BOB_PASSWORD']}\r\n".encode("ascii"),
            b"+OK",
            "message PASS",
        )
        status = send(connection, stream, b"STAT\r\n", b"+OK", "message STAT")
        _status, count, _octets = status.split()
        number = str(int(count))
        command = f"RETR {number}\r\n".encode("ascii")
        send(
            connection,
            stream,
            f"LIST {number}\r\n".encode("ascii"),
            b"-ERR [UTF8]",
            "LIST before UTF8",
        )
        send(
            connection,
            stream,
            f"TOP {number} 1\r\n".encode("ascii"),
            b"-ERR [UTF8]",
            "TOP before UTF8",
        )
        send(connection, stream, command, b"-ERR [UTF8]", "RETR before UTF8")
        send(connection, stream, b"QUIT\r\n", b"+OK", "pre-mode QUIT")
    finally:
        stream.close()
        connection.close()

    connection = socket.create_connection((HOST, PORT), timeout=10)
    stream = connection.makefile("rb")
    try:
        read_line(stream, "enabled UTF8 greeting")
        send(connection, stream, b"STLS\r\n", b"+OK", "enabled UTF8 STLS")
        stream.close()
        connection = context.wrap_socket(connection, server_hostname="localhost")
        stream = connection.makefile("rb")
        send(connection, stream, b"UTF8\r\n", b"+OK", "enable message UTF8")
        send(connection, stream, b"USER bob\r\n", b"+OK", "enabled USER")
        send(
            connection,
            stream,
            f"PASS {os.environ['LITTERKEEP_BOB_PASSWORD']}\r\n".encode("ascii"),
            b"+OK",
            "enabled PASS",
        )
        status = send(connection, stream, b"STAT\r\n", b"+OK", "enabled STAT")
        _status, count, _octets = status.split()
        number = str(int(count))
        command = f"RETR {number}\r\n".encode("ascii")
        send(connection, stream, command, b"+OK", "RETR after UTF8")
        lines = []
        while True:
            line = read_line(stream, "UTF8 message body")
            if line == b".\r\n":
                break
            lines.append(line.removesuffix(b"\r\n"))
        if UTF8_MARKER not in b"\r\n".join(lines):
            raise AssertionError("UTF8 mode did not preserve message octets")
        send(connection, stream, b"QUIT\r\n", b"+OK", "message QUIT")
    finally:
        stream.close()
        connection.close()


def main():
    check_plaintext_upgrade_order()
    check_tls_utf8_mode()
    check_message_download_requires_utf8_mode()
    print("PASS POP3: RFC 6856 UTF8 mode, content guard and STLS ordering")


if __name__ == "__main__":
    main()
