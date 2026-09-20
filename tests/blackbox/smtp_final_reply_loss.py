"""Проверяет durable commit при потере клиентом финального SMTP-ответа."""

import argparse
import imaplib
import smtplib
import socket
import ssl
import time
from enum import Enum


SUBJECT = "final SMTP reply intentionally lost"


class ImapStatus(Enum):
    """Типизированный статус независимого IMAP-клиента."""

    OK = "OK"


class Action(Enum):
    """Фаза, разделяемая полным restart server process."""

    SUBMIT = "submit"
    VERIFY = "verify"


def arguments() -> argparse.Namespace:
    """Читает SMTP/IMAP endpoints и trust anchor."""
    parser = argparse.ArgumentParser()
    parser.add_argument("action", type=Action)
    parser.add_argument("host")
    parser.add_argument("smtp_port", type=int)
    parser.add_argument("imap_port", type=int)
    parser.add_argument("ca_file")
    return parser.parse_args()


def submit_without_reply(options: argparse.Namespace, context: ssl.SSLContext) -> None:
    """Отправляет DATA terminator и закрывает transport до чтения результата."""
    client = smtplib.SMTP(options.host, options.smtp_port, timeout=10)
    client.starttls(context=context)
    client.login("alice", "alice-password")
    code, _ = client.mail("alice@litterkeep.test")
    if code != 250:
        raise AssertionError(f"MAIL rejected: {code}")
    code, _ = client.rcpt("bob@litterkeep.test")
    if code != 250:
        raise AssertionError(f"RCPT rejected: {code}")
    code, _ = client.docmd("DATA")
    if code != 354:
        raise AssertionError(f"DATA rejected: {code}")
    wire = (
        b"From: alice@litterkeep.test\r\n"
        b"To: bob@litterkeep.test\r\n"
        b"Subject: " + SUBJECT.encode("ascii") + b"\r\n\r\n"
        b"The client will not observe the final reply.\r\n.\r\n"
    )
    client.send(wire)
    transport = client.sock
    if transport is None:
        raise AssertionError("SMTP transport disappeared before terminator")
    transport.shutdown(socket.SHUT_RDWR)
    client.close()


def committed(options: argparse.Namespace, context: ssl.SSLContext) -> bool:
    """Ищет Subject новой IMAP-сессией без знания sequence number."""
    with imaplib.IMAP4(options.host, options.imap_port, timeout=10) as client:
        client.starttls(ssl_context=context)
        client.login("bob", "bob-password")
        status, _ = client.select("INBOX", readonly=True)
        if ImapStatus(status) is not ImapStatus.OK:
            raise AssertionError("cannot select INBOX")
        status, values = client.search(None, "SUBJECT", f'"{SUBJECT}"')
        if ImapStatus(status) is not ImapStatus.OK:
            raise AssertionError("SEARCH failed")
        return any(value.strip() for value in values)


def main() -> None:
    """Ожидает worker completion и доказывает опубликованный commit через IMAP."""
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca_file)
    match options.action:
        case Action.SUBMIT:
            submit_without_reply(options, context)
        case Action.VERIFY:
            pass
    for _ in range(20):
        if committed(options, context):
            print("PASS durable commit survived lost final SMTP reply")
            return
        time.sleep(0.1)
    raise AssertionError("message was not committed after SMTP reply loss")


if __name__ == "__main__":
    main()
