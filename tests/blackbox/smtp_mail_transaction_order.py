"""Проверяет RFC 5321 MAIL ordering и сохранение открытого envelope."""

import argparse
from enum import Enum, IntEnum
import imaplib
from pathlib import Path
import smtplib
import ssl
import uuid


class SmtpReply(IntEnum):
    """Числовые коды ответа сценария RFC 5321 §4.1.4."""

    OK = 250
    BAD_SEQUENCE = 503


class ImapReply(Enum):
    """Статус IMAP команд чтения сохранённого письма."""

    OK = "OK"


def password_from(path, account):
    """Читает пароль тестового аккаунта, не выводя значение в отчёт."""
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        match line.split("\t"):
            case [name, password] if name == account:
                return password
            case _:
                continue
    raise ValueError(f"acceptance account is absent: {account}")


def require(actual, expected, context):
    """Сравнивает SMTP/IMAP result с типизированным ожидаемым значением."""
    if actual != expected:
        raise AssertionError(f"{context}: expected {expected}, received {actual}")


def submit_after_rejected_mail(host, port, username, password, subject):
    """После неверного MAIL подтверждает 503 и завершает прежний envelope."""
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    with smtplib.SMTP_SSL(host, port, context=context, timeout=15) as client:
        client.ehlo("rfc5321-order.test")
        client.login(username, password)
        require(client.mail("alice@litterkeep.test")[0], SmtpReply.OK, "MAIL Alice")
        require(
            client.rcpt("bob@litterkeep.test")[0], SmtpReply.OK, "RCPT Bob"
        )
        require(
            client.mail("bob@litterkeep.test")[0],
            SmtpReply.BAD_SEQUENCE,
            "MAIL during active transaction",
        )
        message = (
            "From: alice@litterkeep.test\r\n"
            "To: bob@litterkeep.test\r\n"
            f"Subject: {subject}\r\n\r\n"
            "The original envelope must survive a rejected MAIL.\r\n"
        ).encode("ascii")
        require(client.data(message)[0], SmtpReply.OK, "DATA on preserved envelope")


def verify_envelope(host, port, username, password, subject):
    """Находит доставку у Bob и проверяет исходный Return-Path Alice."""
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    with imaplib.IMAP4_SSL(host, port, ssl_context=context, timeout=15) as client:
        require(client.login(username, password)[0], ImapReply.OK.value, "Bob LOGIN")
        require(client.select("INBOX", readonly=True)[0], ImapReply.OK.value, "SELECT")
        status, matches = client.search(None, "HEADER", "Subject", subject)
        require(status, ImapReply.OK.value, "SEARCH")
        match matches:
            case [sequence_set] if sequence_set:
                pass
            case _:
                raise AssertionError("message from original envelope is missing")
        status, fetched = client.fetch(
            sequence_set, "(BODY.PEEK[HEADER.FIELDS (RETURN-PATH SUBJECT)])"
        )
        require(status, ImapReply.OK.value, "FETCH")
        headers = tuple(
            payload
            for item in fetched
            if isinstance(item, tuple)
            for payload in item
            if isinstance(payload, bytes)
        )
        expected = (b"Return-Path: <alice@litterkeep.test>", subject.encode("ascii"))
        if not all(any(value in header for header in headers) for value in expected):
            raise AssertionError("rejected MAIL changed the active envelope")


def main():
    """Исполняет TLS SMTP transaction и независимо читает результат через IMAP."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, default=1465)
    parser.add_argument("--imap-port", type=int, default=1993)
    parser.add_argument("--credentials", required=True)
    options = parser.parse_args()
    subject = f"RFC5321-order-{uuid.uuid4().hex}"
    submit_after_rejected_mail(
        options.host,
        options.smtp_port,
        "alice",
        password_from(options.credentials, "alice"),
        subject,
    )
    verify_envelope(
        options.host,
        options.imap_port,
        "bob",
        password_from(options.credentials, "bob"),
        subject,
    )
    print("PASS RFC 5321: active MAIL ordering and transaction preservation")


if __name__ == "__main__":
    main()
