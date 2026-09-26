"""Проверяет RFC 2045 QP transport-padding через SMTP и IMAP BINARY."""

import argparse
import enum
import imaplib
import os
import smtplib
import ssl
import uuid


class ImapStatus(enum.Enum):
    """Успешный tagged status независимого IMAP-клиента."""

    OK = "OK"


def require_ok(response: tuple[str, list[object]], operation: str) -> list[object]:
    """Проверяет протокольный статус через enum и возвращает response data."""
    status, data = response
    if ImapStatus(status) is not ImapStatus.OK:
        raise AssertionError(f"{operation} failed: {status!r} {data!r}")
    return data


def options() -> argparse.Namespace:
    """Считывает параметры TLS и acceptance credentials."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--smtp-port", type=int, default=2158)
    parser.add_argument("--imap-port", type=int, default=2146)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    parser.add_argument("--alice-password", default=os.environ.get("LITTERKEEP_ALICE_PASSWORD"))
    parser.add_argument("--bob-password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD"))
    parsed = parser.parse_args()
    if parsed.alice_password is None or parsed.bob_password is None:
        raise ValueError("bootstrap Alice and Bob passwords must be provided through environment")
    return parsed


def fixture(token: str) -> tuple[bytes, bytes]:
    """Смешивает hard-line padding и значимый WSP перед soft-break '='."""
    identity = token.encode("ascii")
    encoded = (
        b"literal-" + identity + b"  \t\r\n"
        b"encoded=20-space  \t\r\n"
        b"soft=20-break  \t=\r\n"
        b"continued\r\n"
        b"joined=20=\r\n"
        b"next\r\n"
    )
    expected = (
        b"literal-" + identity + b"\r\n"
        b"encoded -space\r\n"
        b"soft -break  \tcontinued\r\n"
        b"joined next\r\n"
    )
    raw = (
        b"From: alice@litterkeep.test\r\n"
        b"To: bob@litterkeep.test\r\n"
        b"Subject: quoted-printable padding "
        + identity
        + b"\r\nContent-Type: text/plain; charset=us-ascii\r\n"
        + b"Content-Transfer-Encoding: quoted-printable\r\n\r\n"
        + encoded
    )
    return raw, expected


def deliver(settings: argparse.Namespace, raw: bytes) -> None:
    """Отправляет исходные MIME octets через аутентифицированный SMTP TLS."""
    context = ssl.create_default_context(cafile=settings.ca)
    with smtplib.SMTP("127.0.0.1", settings.smtp_port, timeout=10) as client:
        client.ehlo()
        client.starttls(context=context)
        client.ehlo()
        client.login("alice", settings.alice_password)
        client.sendmail("alice@litterkeep.test", "bob@litterkeep.test", raw)


def fetch_binary(settings: argparse.Namespace) -> bytes:
    """Читает декодированное тело единственного письма через IMAP STARTTLS."""
    context = ssl.create_default_context(cafile=settings.ca)
    with imaplib.IMAP4("127.0.0.1", settings.imap_port, timeout=10) as client:
        client.starttls(ssl_context=context)
        client.login("bob", settings.bob_password)
        require_ok(client.select("INBOX", readonly=True), "INBOX SELECT")
        search = require_ok(client.uid("SEARCH", None, "ALL"), "UID SEARCH")
        identifiers = b" ".join(item for item in search if isinstance(item, bytes)).split()
        if len(identifiers) != 1:
            raise AssertionError(f"expected one message, got {identifiers!r}")
        identifier = next(iter(identifiers), b"")
        if not identifier:
            raise AssertionError("UID SEARCH returned no identifier")
        response = require_ok(
            client.uid("FETCH", identifier, "(BINARY.PEEK[1])"),
            "BINARY.PEEK",
        )
        payloads = [
            value
            for record in response
            if isinstance(record, tuple)
            for value in reversed(record)
            if isinstance(value, bytes)
        ]
        return next(iter(payloads), b"")


def main() -> None:
    settings = options()
    raw, expected = fixture(uuid.uuid4().hex)
    deliver(settings, raw)
    actual = fetch_binary(settings)
    if actual != expected:
        raise AssertionError(f"QP padding projection differs: {actual!r} != {expected!r}")
    print("PASS mime-quoted-printable-padding: SMTP delivery and IMAP BINARY.PEEK")


if __name__ == "__main__":
    main()
