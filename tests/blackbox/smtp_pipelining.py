"""Проверяет RFC 2920 pipeline, упорядоченные ответы и сохранение envelope."""

import base64
from enum import Enum
import imaplib
import os
import re
import socket
import ssl
import uuid


HOST = os.environ.get("LITTERKEEP_SMTP_HOST", "127.0.0.1")
SMTP_PORT = int(os.environ.get("LITTERKEEP_SMTP_PORT", "1587"))
IMAP_PORT = int(os.environ.get("LITTERKEEP_IMAP_PORT", "1143"))
REPLY_PATTERN = re.compile(rb"\A([0-9]{3})([- ])")
TLS_CONTEXT = ssl.create_default_context()
TLS_CONTEXT.check_hostname = False
TLS_CONTEXT.verify_mode = ssl.CERT_NONE


class ImapStatus(Enum):
    """Типизированные статусы, необходимые acceptance клиенту."""

    OK = "OK"
    NO = "NO"
    BAD = "BAD"


def read_reply(stream):
    """Считывает полный SMTP ответ, сохраняя границы multiline reply."""
    lines = []
    response_code = None
    separator = None
    while separator != b" ":
        line = stream.readline()
        match = REPLY_PATTERN.match(line)
        if match is None:
            raise AssertionError(f"malformed SMTP response: {line!r}")
        current_code = match.group(1)
        if response_code is None:
            response_code = current_code
        if current_code != response_code:
            raise AssertionError(f"mixed SMTP reply codes: {lines!r} {line!r}")
        separator = match.group(2)
        lines.append(line)
    return int(response_code), b"".join(lines)


def require_code(reply, expected, context):
    """Сверяет ответ по числовому коду, не по тексту сервера."""
    code, lines = reply
    if code != expected:
        raise AssertionError(f"{context}: expected {expected}, received {lines!r}")
    return lines


def starttls(connection, stream):
    """Переключает независимый socket peer на TLS после 220."""
    stream.write(b"EHLO pipeline-probe.test\r\n")
    capabilities = require_code(read_reply(stream), 250, "pre-TLS EHLO")
    if b"PIPELINING" not in capabilities.upper():
        raise AssertionError(f"PIPELINING absent before STARTTLS: {capabilities!r}")
    stream.write(b"STARTTLS\r\n")
    require_code(read_reply(stream), 220, "STARTTLS")
    stream.close()
    secure = TLS_CONTEXT.wrap_socket(connection, server_hostname=HOST)
    secure_stream = secure.makefile("rwb", buffering=0)
    secure_stream.write(b"EHLO pipeline-probe.test\r\n")
    capabilities = require_code(read_reply(secure_stream), 250, "post-TLS EHLO")
    if b"PIPELINING" not in capabilities.upper():
        raise AssertionError(f"PIPELINING absent after STARTTLS: {capabilities!r}")
    credentials = b"\x00alice\x00" + os.environ["LITTERKEEP_ALICE_PASSWORD"].encode("utf-8")
    response = base64.b64encode(credentials)
    secure_stream.write(b"AUTH PLAIN " + response + b"\r\n")
    require_code(read_reply(secure_stream), 235, "AUTH PLAIN")
    return secure, secure_stream


def pipeline_recipients(stream):
    """Передаёт MAIL, два RCPT и DATA одним write, затем сверяет ответы по порядку."""
    stream.write(
        b"MAIL FROM:<alice@test>\r\n"
        b"RCPT TO:<missing@test>\r\n"
        b"RCPT TO:<bob@test>\r\n"
        b"DATA\r\n"
    )
    stream.flush()
    expected_codes = (250, 550, 250, 354)
    for expected in expected_codes:
        require_code(read_reply(stream), expected, "ordered pipelined command")


def verify_delivery(subject):
    """Читает письмо через независимый IMAP-клиент после SMTP acceptance."""
    with imaplib.IMAP4(HOST, IMAP_PORT, timeout=15) as mailbox:
        mailbox.starttls(ssl_context=TLS_CONTEXT)
        password = os.environ["LITTERKEEP_BOB_PASSWORD"]
        login_status, _ = mailbox.login("bob", password)
        if ImapStatus(login_status) is not ImapStatus.OK:
            raise AssertionError("Bob IMAP login failed")
        select_status, _ = mailbox.select("INBOX", readonly=True)
        if ImapStatus(select_status) is not ImapStatus.OK:
            raise AssertionError("Bob INBOX selection failed")
        status, results = mailbox.search(None, "HEADER", "Subject", f'"{subject}"')
        message_ids = next(iter(results), b"")
        if status != "OK" or not message_ids:
            raise AssertionError(f"pipelined delivery missing in IMAP: {status} {results!r}")


def reject_data_after_all_recipients_fail():
    """DATA не становится положительным после полностью отклонённых RCPT."""
    connection = socket.create_connection((HOST, SMTP_PORT), timeout=15)
    stream = connection.makefile("rwb", buffering=0)
    require_code(read_reply(stream), 220, "all-rejected SMTP greeting")
    connection, stream = starttls(connection, stream)
    stream.write(
        b"MAIL FROM:<alice@test>\r\n"
        b"RCPT TO:<missing@test>\r\n"
        b"DATA\r\n"
    )
    stream.flush()
    require_code(read_reply(stream), 250, "pipelined MAIL before rejected DATA")
    require_code(read_reply(stream), 550, "pipelined invalid recipient")
    require_code(read_reply(stream), 503, "DATA without an accepted recipient")
    stream.write(b"NOOP\r\n")
    require_code(read_reply(stream), 250, "command after rejected pipelined DATA")
    stream.write(b"QUIT\r\n")
    require_code(read_reply(stream), 221, "QUIT after rejected pipelined DATA")
    stream.close()
    connection.close()


def main():
    """Проверяет capability и полный DATA/IMAP путь с pipeline."""
    subject = "RFC2920 " + uuid.uuid4().hex
    connection = socket.create_connection((HOST, SMTP_PORT), timeout=15)
    stream = connection.makefile("rwb", buffering=0)
    require_code(read_reply(stream), 220, "SMTP greeting")
    connection, stream = starttls(connection, stream)
    message = (
        "From: alice@test\r\n"
        "To: bob@test\r\n"
        f"Subject: {subject}\r\n\r\n"
        "Ordered PIPELINING replies preserve a later valid recipient.\r\n"
    ).encode("ascii")
    pipeline_recipients(stream)
    stream.write(message + b".\r\n")
    require_code(read_reply(stream), 250, "pipelined transaction delivery")
    stream.write(b"NOOP\r\n")
    require_code(read_reply(stream), 250, "command after DATA completion")
    stream.write(b"QUIT\r\n")
    require_code(read_reply(stream), 221, "QUIT")
    stream.close()
    connection.close()
    verify_delivery(subject)
    reject_data_after_all_recipients_fail()
    print(
        "PASS smtp-pipelining: ordered valid/invalid RCPT, "
        "DATA gating and durable IMAP delivery"
    )


if __name__ == "__main__":
    main()
