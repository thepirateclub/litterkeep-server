"""Проверяет RFC WSP в Date header через независимый IMAP SEARCH peer."""

import argparse
import enum
import imaplib
import os
import ssl
import uuid


class ImapStatus(enum.Enum):
    """Tagged status, который acceptance принимает как успешный."""

    OK = "OK"


def require_ok(response: tuple[str, object], operation: str) -> object:
    """Прерывает сценарий на первом отказе протокола."""
    status, data = response
    if ImapStatus(status) is not ImapStatus.OK:
        raise AssertionError(f"{operation}: {status!r} {data!r}")
    return data


def date_messages() -> tuple[bytes, ...]:
    """Возвращает валидную дату и четыре NBSP CFWS-подмены."""
    common = b"From: alice@example.test\r\nTo: bob@example.test\r\n"
    dates = (
        b"2 Feb 2026 12:30:00 +0000",
        b"\xc2\xa02 Feb 2026 12:30:00 +0000",
        b"Mon\xc2\xa0, 2 Feb 2026 12:30:00 +0000",
        b"Mon,\xc2\xa02 Feb 2026 12:30:00 +0000",
        b"2 Feb 2026 12:30:00 +0000\xc2\xa0",
    )
    return tuple(common + b"Date: " + date + b"\r\n\r\nbody\r\n" for date in dates)


def main() -> None:
    """APPEND-ит fixtures и проверяет SENTON по RFC 5322 Date field."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=2148)
    parser.add_argument("--ca", default="data/g17-imap-sent-date-wsp/pki/ca.pem")
    parser.add_argument("--password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD"))
    options = parser.parse_args()
    if options.password is None:
        raise ValueError("provide --password or LITTERKEEP_BOB_PASSWORD")
    context = ssl.create_default_context(cafile=options.ca)
    mailbox = f"SentDateWsp-{uuid.uuid4().hex}"
    with imaplib.IMAP4(options.host, options.port, timeout=10) as client:
        client.starttls(ssl_context=context)
        require_ok(client.login("bob", options.password), "LOGIN")
        require_ok(client.create(mailbox), "CREATE")
        for message in date_messages():
            require_ok(client.append(mailbox, None, None, message), "APPEND")
        require_ok(client.select(mailbox, readonly=True), "SELECT")
        result = require_ok(client.search(None, "SENTON", "2-Feb-2026"), "SEARCH")
    if result != [b"1"]:
        raise AssertionError(f"SENTON matched invalid Date headers: {result!r}")
    print("PASS IMAP SENTON rejects NBSP outside RFC 5322 WSP")


if __name__ == "__main__":
    main()
