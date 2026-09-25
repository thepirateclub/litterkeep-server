"""Проверяет POP3 512-octet status lines и длинные RETR message lines."""

import os
import base64
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


def read_capabilities(connection, stream, context):
    connection.sendall(b"CAPA\r\n")
    status = read_line(stream, f"{context} CAPA status")
    if not status.startswith(b"+OK") or len(status) > 512:
        raise AssertionError(f"invalid {context} CAPA status: {status!r}")
    capabilities = []
    while True:
        line = read_line(stream, f"{context} CAPA line")
        if len(line) > 512:
            raise AssertionError(f"{context} CAPA line exceeds 512 octets")
        if line == b".\r\n":
            return capabilities
        capabilities.append(line.rstrip(b"\r\n"))


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

        auth_capabilities = read_capabilities(connection, stream, "authorization")
        if b"RESP-CODES" not in auth_capabilities:
            raise AssertionError("RESP-CODES capability missing before authentication")

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

        transaction_capabilities = read_capabilities(connection, stream, "transaction")
        if b"RESP-CODES" not in transaction_capabilities:
            raise AssertionError("RESP-CODES capability missing after authentication")
        verify_maildrop_in_use(tls_context)
        send(connection, stream, b"QUIT\r\n", b"+OK", "QUIT")
    finally:
        stream.close()
        connection.close()


def verify_sasl_plain(tls_context, with_continuation):
    connection = socket.create_connection((HOST, POP3_PORT), timeout=15)
    stream = connection.makefile("rb")
    try:
        read_line(stream, "SASL POP3 greeting")
        clear_capabilities = read_capabilities(connection, stream, "plaintext")
        if b"SASL PLAIN" in clear_capabilities:
            raise AssertionError("plaintext CAPA must not advertise SASL PLAIN")
        send(
            connection,
            stream,
            b"AUTH PLAIN\r\n",
            b"-ERR STLS required",
            "plaintext AUTH PLAIN rejection",
        )
        send(connection, stream, b"STLS\r\n", b"+OK", "SASL STLS")
        stream.close()
        connection = tls_context.wrap_socket(connection, server_hostname="localhost")
        stream = connection.makefile("rb")
        encrypted_capabilities = read_capabilities(connection, stream, "TLS authorization")
        if b"SASL PLAIN" not in encrypted_capabilities:
            raise AssertionError("TLS CAPA did not advertise SASL PLAIN")
        if b"STLS" in encrypted_capabilities:
            raise AssertionError("TLS CAPA must not continue to advertise STLS")

        password = os.environ["LITTERKEEP_BOB_PASSWORD"].encode("ascii")
        response = base64.b64encode(b"\x00bob\x00" + password)
        if with_continuation:
            send(connection, stream, b"AUTH PLAIN\r\n", b"+ ", "AUTH PLAIN challenge")
            send(connection, stream, b"*\r\n", b"-ERR", "SASL cancellation")
            send(connection, stream, b"AUTH SCRAM-SHA-256 token\r\n", b"-ERR", "unsupported SASL mechanism")
            send(connection, stream, b"AUTH PLAIN\r\n", b"+ ", "AUTH PLAIN retry")
            connection.sendall(response + b"\r\n")
            success = read_line(stream, "SASL PLAIN continuation result")
            if not success.startswith(b"+OK"):
                raise AssertionError(f"AUTH PLAIN continuation failed: {success!r}")
        else:
            connection.sendall(b"AUTH PLAIN " + response + b"\r\n")
            success = read_line(stream, "SASL PLAIN initial-response result")
            if not success.startswith(b"+OK"):
                raise AssertionError(f"AUTH PLAIN initial response failed: {success!r}")

        send(connection, stream, b"CAPA\r\n", b"+OK", "CAPA after SASL authentication")
        transaction_capabilities = []
        while True:
            line = read_line(stream, "transaction SASL capability")
            if line == b".\r\n":
                break
            transaction_capabilities.append(line.rstrip(b"\r\n"))
        if b"SASL PLAIN" not in transaction_capabilities:
            raise AssertionError("SASL capability disappeared after successful AUTH")
        send(connection, stream, b"AUTH PLAIN invalid\r\n", b"-ERR", "AUTH after success")
        send(connection, stream, b"QUIT\r\n", b"+OK", "SASL QUIT")
    finally:
        stream.close()
        connection.close()


def verify_failed_sasl_retry(tls_context):
    connection = socket.create_connection((HOST, POP3_PORT), timeout=15)
    stream = connection.makefile("rb")
    try:
        read_line(stream, "retry POP3 greeting")
        send(connection, stream, b"STLS\r\n", b"+OK", "retry STLS")
        stream.close()
        connection = tls_context.wrap_socket(connection, server_hostname="localhost")
        stream = connection.makefile("rb")
        read_capabilities(connection, stream, "retry authorization")
        wrong = base64.b64encode(b"\x00bob\x00wrong-password")
        send(connection, stream, b"AUTH PLAIN " + wrong + b"\r\n", b"-ERR", "failed AUTH PLAIN")
        password = os.environ["LITTERKEEP_BOB_PASSWORD"].encode("ascii")
        correct = base64.b64encode(b"\x00bob\x00" + password)
        send(connection, stream, b"AUTH PLAIN " + correct + b"\r\n", b"+OK", "AUTH PLAIN retry")
        send(connection, stream, b"QUIT\r\n", b"+OK", "retry QUIT")
    finally:
        stream.close()
        connection.close()


def verify_maildrop_in_use(tls_context):
    connection = socket.create_connection((HOST, POP3_PORT), timeout=15)
    stream = connection.makefile("rb")
    try:
        read_line(stream, "second POP3 greeting")
        send(connection, stream, b"STLS\r\n", b"+OK", "second STLS")
        stream.close()
        connection = tls_context.wrap_socket(connection, server_hostname="localhost")
        stream = connection.makefile("rb")
        read_capabilities(connection, stream, "second authorization")
        send(connection, stream, b"USER bob\r\n", b"+OK", "second USER")
        send(
            connection,
            stream,
            b"USER alice\r\n",
            b"-ERR invalid command for state",
            "repeated USER before PASS",
        )
        send(
            connection,
            stream,
            b"PASS without-user\r\n",
            b"-ERR USER required",
            "rejected USER clears the previous selection",
        )
        send(connection, stream, b"USER bob\r\n", b"+OK", "USER retry after rejection")
        send(
            connection,
            stream,
            b"PASS wrong-password\r\n",
            b"-ERR authentication failed",
            "failed PASS",
        )
        send(
            connection,
            stream,
            b"PASS retry-without-user\r\n",
            b"-ERR USER required",
            "PASS requires a new USER after failure",
        )
        send(connection, stream, b"USER bob\r\n", b"+OK", "USER after failed PASS")
        send(
            connection,
            stream,
            f"PASS {os.environ['LITTERKEEP_BOB_PASSWORD']}\r\n".encode("ascii"),
            b"-ERR [IN-USE] ",
            "RFC 2449 locked maildrop response",
        )
    finally:
        stream.close()
        connection.close()


def main():
    tls_context = ssl.create_default_context()
    tls_context.check_hostname = False
    tls_context.verify_mode = ssl.CERT_NONE
    deliver_long_line(tls_context)
    pop3_round_trip(tls_context)
    verify_sasl_plain(tls_context, with_continuation=False)
    verify_sasl_plain(tls_context, with_continuation=True)
    verify_failed_sasl_retry(tls_context)
    print("PASS POP3: SASL PLAIN, bounded lines, long RETR and RESP-CODES")


if __name__ == "__main__":
    main()
