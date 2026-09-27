"""Проверяет null reverse-path в аутентифицированной SMTP submission."""

import argparse
from enum import Enum, IntEnum
import imaplib
from pathlib import Path
import smtplib
import ssl
import uuid


class ReplyCode(IntEnum):
    """Основные статусы команд узкого RFC 6409 сценария."""

    ACCEPTED = 250
    DATA = 354
    SENDER_DENIED = 550


class Account(Enum):
    """Локальные acceptance аккаунты, участвующие в маршруте письма."""

    ALICE = "alice"
    BOB = "bob"


class ImapStatus(Enum):
    """Результаты чтения доставленного письма через IMAP."""

    OK = "OK"


def password_from(path, account):
    """Читает bootstrap пароль без вывода его в консоль."""
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        match line.split("\t"):
            case [name, password] if name == account.value:
                return password
            case _:
                continue
    raise ValueError(f"acceptance account is absent: {account.value}")


def submit_null_sender(host, port, credentials, subject):
    """Проверяет MAIL FROM:<> после TLS AUTH и локально доставляет письмо."""
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    with smtplib.SMTP_SSL(host, port, context=context, timeout=15) as client:
        client.ehlo("rfc6409-null-sender.test")
        client.login(Account.ALICE.value, credentials)
        foreign_code, _ = client.mail("bob@litterkeep.test")
        if foreign_code != ReplyCode.SENDER_DENIED:
            raise AssertionError("authenticated sender ownership policy was weakened")
        mail_code, _ = client.mail("")
        if mail_code != ReplyCode.ACCEPTED:
            raise AssertionError(f"authenticated MAIL FROM:<> rejected: {mail_code}")
        rcpt_code, _ = client.rcpt("bob@litterkeep.test")
        if rcpt_code != ReplyCode.ACCEPTED:
            raise AssertionError(f"local RCPT rejected: {rcpt_code}")
        message = (
            "From: alice@litterkeep.test\r\n"
            "To: bob@litterkeep.test\r\n"
            f"Subject: {subject}\r\n\r\n"
            "RFC 6409 null reverse-path acceptance\r\n"
        ).encode("ascii")
        data_code, _ = client.data(message)
        if data_code != ReplyCode.ACCEPTED:
            raise AssertionError(f"message DATA failed: {data_code}")


def verify_return_path(host, port, credentials, subject):
    """Проверяет Return-Path: <> в сохранённом сообщении Bob."""
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    with imaplib.IMAP4_SSL(host, port, ssl_context=context, timeout=15) as client:
        status, _ = client.login(Account.BOB.value, credentials)
        if status != ImapStatus.OK.value:
            raise AssertionError("Bob IMAP authentication failed")
        status, _ = client.select("INBOX", readonly=True)
        if status != ImapStatus.OK.value:
            raise AssertionError("Bob INBOX selection failed")
        status, found = client.search(None, "HEADER", "Subject", subject)
        if status != ImapStatus.OK.value:
            raise AssertionError("IMAP header search failed")
        match found:
            case [sequence_set] if sequence_set:
                pass
            case _:
                raise AssertionError("null-sender message was not delivered")
        status, fetched = client.fetch(
            sequence_set, "(BODY.PEEK[HEADER.FIELDS (RETURN-PATH SUBJECT)])"
        )
        if status != ImapStatus.OK.value:
            raise AssertionError("IMAP header fetch failed")
        headers = tuple(
            payload
            for item in fetched
            if isinstance(item, tuple)
            for payload in item
            if isinstance(payload, bytes)
        )
        required_headers = (b"Return-Path: <>", subject.encode("ascii"))
        if not all(any(header_value in payload for payload in headers) for header_value in required_headers):
            raise AssertionError("delivered message did not preserve null Return-Path")


def main():
    """Выполняет независимую submission -> mailbox проверку."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, default=1465)
    parser.add_argument("--imap-port", type=int, default=1993)
    parser.add_argument("--credentials", required=True)
    options = parser.parse_args()
    alice = password_from(options.credentials, Account.ALICE)
    bob = password_from(options.credentials, Account.BOB)
    subject = f"RFC6409-null-{uuid.uuid4().hex}"
    submit_null_sender(options.host, options.smtp_port, alice, subject)
    verify_return_path(options.host, options.imap_port, bob, subject)
    print("PASS RFC 6409: authenticated null reverse-path is accepted and preserved")


if __name__ == "__main__":
    main()
