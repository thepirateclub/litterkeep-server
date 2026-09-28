"""Проверяет RFC 5322 Date whitespace и obsolete timezone через IMAP SEARCH."""

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
    try:
        parsed_status = ImapStatus(status)
    except ValueError as error:
        raise AssertionError(f"{operation}: {status!r} {data!r}") from error
    if parsed_status is not ImapStatus.OK:
        raise AssertionError(f"{operation}: {status!r} {data!r}")
    return data


def date_messages() -> tuple[bytes, ...]:
    """Строит numeric, obsolete-zone, invalid-zone и NBSP fixtures."""
    common = b"From: alice@example.test\r\nTo: bob@example.test\r\n"
    dates = (
        b"2 Feb 2026 12:30:00 +0000",
        b"\xc2\xa02 Feb 2026 12:30:00 +0000",
        b"Mon\xc2\xa0, 2 Feb 2026 12:30:00 +0000",
        b"Mon,\xc2\xa02 Feb 2026 12:30:00 +0000",
        b"2 Feb 2026 12:30:00 +0000\xc2\xa0",
        b"Sun, 1 Feb 2026 23:30:00 EST",
        b"Mon, 2 Feb 2026 00:30:00 EDT",
        b"Mon, 2 Feb 2026 12:30:00 A",
        b"Mon, 2 Feb 2026 12:30:00 J",
        b"Mon, 2 Feb 2026 12:30:00 XYZ",
        b"Mon, 2 Feb 2026 12:30:00 UNKNOWN",
    )
    return tuple(common + b"Date: " + date + b"\r\n\r\nbody\r\n" for date in dates)


def main() -> None:
    """APPEND-ит fixtures и сверяет SENTON для obsolete timezone policy."""
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
        require_ok(client._simple_command("ENABLE", "UTF8=ACCEPT"), "ENABLE UTF8=ACCEPT")
        require_ok(client.create(mailbox), "CREATE")
        for message in date_messages():
            require_ok(client.append(mailbox, None, None, message), "APPEND")
        require_ok(client.select(mailbox, readonly=True), "SELECT")
        result = require_ok(client.search(None, "SENTON", "2-Feb-2026"), "SEARCH")
    if result != [b"1 6 7 8 10"]:
        raise AssertionError(f"SENTON Date zone projection is incorrect: {result!r}")
    print("PASS IMAP SENTON numeric and RFC 5322 obsolete timezone policy")


if __name__ == "__main__":
    main()
