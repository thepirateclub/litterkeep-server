"""Независимо проверяет RFC 6856 UTF8 command и порядок STLS."""

import imaplib
import os
import smtplib
import socket
import ssl


HOST = os.environ.get("LITTERKEEP_HOST", "127.0.0.1")
PORT = int(os.environ.get("LITTERKEEP_POP3_PORT", "1110"))
SMTP_PORT = int(os.environ.get("LITTERKEEP_SMTP_PORT", "2525"))
IMAP_PORT = int(os.environ.get("LITTERKEEP_IMAP_PORT", "143"))
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
        if b"UTF8" not in values or b"UTF8 USER" in values or b"LANG" not in values:
            raise AssertionError(f"unexpected plaintext UTF8 capability: {values!r}")
        send(connection, stream, b"LANG\r\n", b"+OK", "LANG listing")
        listing = read_line(stream, "LANG listing language")
        terminator = read_line(stream, "LANG listing terminator")
        if listing != b"en English\r\n" or terminator != b".\r\n":
            raise AssertionError(f"unexpected LANG listing: {listing!r} {terminator!r}")
        send(connection, stream, b"LANG EN\r\n", b"+OK en ", "LANG English")
        send(connection, stream, b"LANG *\r\n", b"+OK en ", "LANG preferred")
        send(connection, stream, b"LANG de\r\n", b"-ERR", "unsupported LANG")
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
        if b"UTF8" not in values or b"UTF8 USER" in values or b"LANG" not in values:
            raise AssertionError(f"unexpected TLS UTF8 capability: {values!r}")
        send(connection, stream, b"UTF8\r\n", b"+OK", "TLS UTF8 command")
        send(connection, stream, b"QUIT\r\n", b"+OK", "TLS QUIT")
    finally:
        stream.close()
        connection.close()


def check_message_download_requires_utf8_mode():
    opaque_message = (
        b"From: alice@test\r\nTo: bob@test\r\nSubject: opaque high-bit octet\r\n"
        b"MIME-Version: 1.0\r\nContent-Type: application/octet-stream\r\n\r\n\xff\r\n"
    )
    mailbox = imaplib.IMAP4(HOST, IMAP_PORT)
    try:
        mailbox.starttls(ssl_context=ssl_context())
        status, _ = mailbox.login("bob", os.environ["LITTERKEEP_BOB_PASSWORD"])
        if status != "OK":
            raise AssertionError("IMAP login failed before opaque-octet fixture APPEND")
        status, _ = mailbox.append("INBOX", None, None, opaque_message)
        if status != "OK":
            raise AssertionError("IMAP APPEND rejected opaque high-bit fixture")
    finally:
        mailbox.logout()

    message = (
        b"From: alice@test\r\nTo: bob@test\r\nSubject: UTF8 mode test\r\n"
        b"Content-Type: text/plain; charset=utf-8\r\n"
        b"Content-Transfer-Encoding: 8bit\r\n\r\n"
        + UTF8_MARKER
        + b"\r\n"
    )
    context = ssl_context()
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
        transaction_capabilities = capabilities(connection, stream, "transaction")
        if b"LANG" not in transaction_capabilities:
            raise AssertionError("LANG capability disappeared in TRANSACTION state")
        send(connection, stream, b"LANG EN\r\n", b"+OK en ", "transaction LANG")
        status = send(connection, stream, b"STAT\r\n", b"+OK", "message STAT")
        _status, count, _octets = status.split()
        number = str(int(count))
        opaque_number = str(max(1, int(count) - 1))
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
        opaque_rejection = b"-ERR non-ASCII message requires UTF8 mode"
        for verb in (b"LIST", b"TOP", b"RETR"):
            argument = b" 1" if verb == b"TOP" else b""
            send(
                connection,
                stream,
                verb + b" " + opaque_number.encode("ascii") + argument + b"\r\n",
                opaque_rejection,
                f"{verb.decode('ascii')} opaque octets before UTF8",
            )
        send(connection, stream, b"QUIT\r\n", b"+OK", "pre-mode QUIT")
    finally:
        stream.close()
        connection.close()


def check_binary_cte_rejected():
    raw_message = (
        b"From: alice@test\r\nTo: bob@test\r\nSubject: binary CTE policy\r\n"
        b"MIME-Version: 1.0\r\nContent-Type: multipart/report; boundary=policy\r\n\r\n"
        b"--policy\r\nContent-Type: text/plain\r\n\r\nnotice\r\n"
        b"--policy\r\nContent-Type: application/octet-stream\r\n"
        b"Content-Transfer-Encoding: binary\r\n\r\n"
        b"opaque\x00octets\r\n--policy--\r\n"
    )
    malformed_message = (
        b"From: alice@test\r\nTo: bob@test\r\nSubject: unverifiable binary CTE\r\n"
        b"MIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=broken\r\n\r\n"
        b"--broken\r\nContent-Type: application/octet-stream\r\n"
        b"Content-Transfer-Encoding: binary\r\n\r\nopaque\x00octets\r\n"
    )
    context = ssl_context()
    mailbox = imaplib.IMAP4(HOST, IMAP_PORT)
    try:
        mailbox.starttls(ssl_context=context)
        status, _ = mailbox.login("bob", os.environ["LITTERKEEP_BOB_PASSWORD"])
        if status != "OK":
            raise AssertionError("IMAP login failed before MIME fixture APPEND")
        status, _ = mailbox.append("INBOX", None, None, raw_message)
        if status != "OK":
            raise AssertionError("IMAP APPEND rejected the MIME binary CTE fixture")
        status, _ = mailbox.append("INBOX", None, None, malformed_message)
        if status != "OK":
            raise AssertionError("IMAP APPEND rejected malformed multipart fixture")
    finally:
        mailbox.logout()

    connection = socket.create_connection((HOST, PORT), timeout=10)
    stream = connection.makefile("rb")
    try:
        read_line(stream, "binary CTE POP3 greeting")
        send(connection, stream, b"STLS\r\n", b"+OK", "binary CTE STLS")
        stream.close()
        connection = context.wrap_socket(connection, server_hostname="localhost")
        stream = connection.makefile("rb")
        send(connection, stream, b"UTF8\r\n", b"+OK", "binary CTE UTF8")
        send(connection, stream, b"USER bob\r\n", b"+OK", "binary CTE USER")
        send(
            connection,
            stream,
            f"PASS {os.environ['LITTERKEEP_BOB_PASSWORD']}\r\n".encode("ascii"),
            b"+OK",
            "binary CTE PASS",
        )
        status = send(connection, stream, b"STAT\r\n", b"+OK", "binary CTE STAT")
        _ok, count, _octets = status.split()
        message_count = int(count)
        if message_count < 2:
            raise AssertionError(f"expected both CTE fixtures in the maildrop: {message_count}")
        binary_number = message_count - 1
        binary_retr = f"RETR {binary_number}\r\n".encode("ascii")
        rejection = b"-ERR MIME binary transfer encoding is not permitted"
        send(connection, stream, binary_retr, rejection, "binary CTE RETR")
        top_command = f"TOP {binary_number} 2\r\n".encode("ascii")
        send(connection, stream, top_command, rejection, "binary CTE TOP")
        unverifiable_rejection = b"-ERR MIME structure cannot be verified for POP3"
        malformed_retr = f"RETR {message_count}\r\n".encode("ascii")
        send(connection, stream, malformed_retr, unverifiable_rejection, "unverifiable MIME RETR")
        malformed_top = f"TOP {message_count} 2\r\n".encode("ascii")
        send(connection, stream, malformed_top, unverifiable_rejection, "unverifiable MIME TOP")
        send(connection, stream, b"QUIT\r\n", b"+OK", "binary CTE QUIT")
    finally:
        stream.close()
        connection.close()


def ssl_context():
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    return context

def main():
    check_plaintext_upgrade_order()
    check_tls_utf8_mode()
    check_message_download_requires_utf8_mode()
    check_binary_cte_rejected()
    print("PASS POP3: RFC 6856 UTF8 mode, content guard and STLS ordering")


if __name__ == "__main__":
    main()
