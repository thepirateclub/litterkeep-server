"""Проверяет implicit-TLS SMTP доставку и последующее peer shutdown."""

import argparse
import base64
from enum import Enum, IntEnum
import imaplib
from pathlib import Path
import socket
import ssl
import uuid


class SmtpCode(IntEnum):
    """Числовые статусы SMTP-команд этого acceptance сценария."""

    GREETING = 220
    OK = 250
    AUTHENTICATED = 235
    DATA = 354


class Account(Enum):
    """Bootstrap учётные записи независимой сквозной проверки."""

    ALICE = "alice"
    BOB = "bob"


class ImapStatus(Enum):
    """Результаты команд IMAP, используемые при проверке доставки."""

    OK = "OK"


def password_from(path, account):
    """Читает пароль bootstrap-аккаунта, не выводя секрет в сообщения."""
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        match line.split("\t"):
            case [name, password] if name == account.value:
                return password
            case _:
                continue
    raise ValueError(f"acceptance account is absent: {account.value}")


def read_reply(stream, expected):
    """Читает весь multiline SMTP ответ и сверяет его основной код."""
    status = None
    separator = None
    while separator != b" ":
        line = stream.readline()
        match line[:3], line[3:4]:
            case raw_code, raw_separator if len(raw_code) == 3:
                status = raw_code if status is None else status
                if raw_code != status:
                    raise AssertionError(f"mixed SMTP reply codes: {line!r}")
                separator = raw_separator
            case _:
                raise AssertionError(f"malformed SMTP reply: {line!r}")
    if status != str(expected.value).encode("ascii"):
        raise AssertionError(f"expected SMTP {expected.value}, received {status!r}")


def submit_then_close(host, port, username, password, subject):
    """Отправляет DATA, читает 250 и закрывает TLS без SMTP QUIT."""
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    transport = socket.create_connection((host, port), timeout=15)
    connection = context.wrap_socket(transport, server_hostname=host)
    connection.settimeout(15)
    stream = connection.makefile("rwb", buffering=0)
    read_reply(stream, SmtpCode.GREETING)
    stream.write(b"EHLO rfc8314-peer.test\r\n")
    read_reply(stream, SmtpCode.OK)
    credentials = base64.b64encode(b"\x00" + username.encode() + b"\x00" + password.encode())
    stream.write(b"AUTH PLAIN " + credentials + b"\r\n")
    read_reply(stream, SmtpCode.AUTHENTICATED)
    stream.write(b"MAIL FROM:<alice@litterkeep.test>\r\n")
    read_reply(stream, SmtpCode.OK)
    stream.write(b"RCPT TO:<bob@litterkeep.test>\r\n")
    read_reply(stream, SmtpCode.OK)
    stream.write(b"DATA\r\n")
    read_reply(stream, SmtpCode.DATA)
    message = (
        b"From: alice@litterkeep.test\r\n"
        b"To: bob@litterkeep.test\r\n"
        + b"Subject: " + subject.encode("ascii") + b"\r\n\r\n"
        + b"close_notify after complete DATA\r\n.\r\n"
    )
    stream.write(message)
    read_reply(stream, SmtpCode.OK)
    stream.close()
    raw = connection.unwrap()
    raw.close()


def verify_delivery(host, port, username, password, subject):
    """Входит как Bob и ищет точный subject в сохранённом INBOX."""
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    with imaplib.IMAP4_SSL(host, port, ssl_context=context, timeout=15) as client:
        status, _ = client.login(username, password)
        if status != ImapStatus.OK.value:
            raise AssertionError("Bob IMAP authentication failed")
        status, _ = client.select("INBOX", readonly=True)
        if status != ImapStatus.OK.value:
            raise AssertionError("Bob INBOX selection failed")
        status, data = client.search(None, "HEADER", "Subject", subject)
        if status != ImapStatus.OK.value or not any(data):
            raise AssertionError("message was not committed to Bob's INBOX")


def main():
    """Выполняет SMTP peer close и подтверждает durable delivery через IMAP."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, default=1465)
    parser.add_argument("--imap-port", type=int, default=1993)
    parser.add_argument("--credentials", required=True)
    options = parser.parse_args()
    subject = f"RFC8314-close-{uuid.uuid4().hex}"
    submit_then_close(
        options.host,
        options.smtp_port,
        Account.ALICE.value,
        password_from(options.credentials, Account.ALICE),
        subject,
    )
    verify_delivery(
        options.host,
        options.imap_port,
        Account.BOB.value,
        password_from(options.credentials, Account.BOB),
        subject,
    )
    print("PASS RFC 8314: implicit SMTP delivery and peer close_notify")


if __name__ == "__main__":
    main()
