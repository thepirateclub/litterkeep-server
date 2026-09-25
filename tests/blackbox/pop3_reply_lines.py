"""Проверяет POP3 512-octet status lines и длинные RETR message lines."""

import os
import smtplib
import socket
import ssl


HOST = os.environ.get("LITTERKEEP_HOST", "127.0.0.1")
SMTP_PORT = int(os.environ.get("LITTERKEEP_SMTP_PORT", "2525"))
POP3_PORT = int(os.environ.get("LITTERKEEP_POP3_PORT", "1110"))
MESSAGE_LINE = b"x" * 700


def read_line(stream, context):
    line = stream.readline()
    if not line.endswith(b"\r\n"):
        raise AssertionError(f"{context}: response is not CRLF terminated: {line!r}")
    return line


def send(connection, stream, command, prefix, context):
    connection.sendall(command)
    line = read_line(stream, context)
    if not line.startswith(prefix):
        raise AssertionError(f"{context}: expected {prefix!r}, received {line!r}")
    return line


def deliver_long_line(tls_context):
    message = (
        b"From: alice@test\r\nTo: bob@test\r\nSubject: long POP3 line\r\n\r\n"
        + MESSAGE_LINE
        + b"\r\n"
    )
    with smtplib.SMTP(HOST, SMTP_PORT, timeout=15) as client:
        client.starttls(context=tls_context)
        client.login("alice", os.environ["LITTERKEEP_ALICE_PASSWORD"])
        refused = client.sendmail("alice@test", ["bob@test"], message)
        if refused:
            raise AssertionError(f"SMTP refused local recipient: {refused!r}")


def pop3_round_trip(tls_context):
    connection = socket.create_connection((HOST, POP3_PORT), timeout=15)
    stream = connection.makefile("rb")
    try:
        greeting = read_line(stream, "POP3 greeting")
        if not greeting.startswith(b"+OK") or len(greeting) > 512:
            raise AssertionError(f"invalid greeting line length/status: {greeting!r}")
        send(connection, stream, b"STLS\r\n", b"+OK", "STLS response")
        stream.close()
        connection = tls_context.wrap_socket(connection, server_hostname="localhost")
        stream = connection.makefile("rb")

        connection.sendall(b"CAPA\r\n")
        capa_status = read_line(stream, "CAPA status")
        if not capa_status.startswith(b"+OK") or len(capa_status) > 512:
            raise AssertionError(f"invalid CAPA status: {capa_status!r}")
        while True:
            line = read_line(stream, "CAPA line")
            if len(line) > 512:
                raise AssertionError(f"CAPA line exceeds 512 octets: {len(line)}")
            if line == b".\r\n":
                break

        send(connection, stream, b"USER bob\r\n", b"+OK", "USER")
        send(
            connection,
            stream,
            f"PASS {os.environ['LITTERKEEP_BOB_PASSWORD']}\r\n".encode("ascii"),
            b"+OK",
            "PASS",
        )
        for command in (b"STAT\r\n", b"LIST\r\n", b"UIDL\r\n"):
            connection.sendall(command)
            status = read_line(stream, "POP3 status")
            if not status.startswith(b"+OK") or len(status) > 512:
                raise AssertionError(f"invalid status response: {status!r}")
            if command != b"STAT\r\n":
                while True:
                    line = read_line(stream, "POP3 listing")
                    if len(line) > 512:
                        raise AssertionError(f"listing line exceeds bound: {len(line)}")
                    if line == b".\r\n":
                        break

        connection.sendall(b"RETR 1\r\n")
        status = read_line(stream, "RETR status")
        if not status.startswith(b"+OK") or len(status) > 512:
            raise AssertionError(f"invalid RETR status: {status!r}")
        message_lines = []
        while True:
            line = read_line(stream, "RETR message line")
            if line == b".\r\n":
                break
            message_lines.append(line.removesuffix(b"\r\n"))
        if MESSAGE_LINE not in message_lines:
            raise AssertionError("RETR changed or rejected the 700-octet RFC message line")
        send(connection, stream, b"QUIT\r\n", b"+OK", "QUIT")
    finally:
        stream.close()
        connection.close()


def main():
    tls_context = ssl.create_default_context()
    tls_context.check_hostname = False
    tls_context.verify_mode = ssl.CERT_NONE
    deliver_long_line(tls_context)
    pop3_round_trip(tls_context)
    print("PASS pop3-reply-lines: status/list lines bounded at 512; 700-octet RETR body line preserved")


if __name__ == "__main__":
    main()
