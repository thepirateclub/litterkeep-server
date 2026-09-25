"""Проверяет RFC 2045 comments в Content-Transfer-Encoding через wire."""

import argparse
import base64
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
    """Проверяет tagged status enum и возвращает response data."""
    status, data = response
    if ImapStatus(status) is not ImapStatus.OK:
        raise AssertionError(f"{operation} failed: {status!r} {data!r}")
    return data


def options() -> argparse.Namespace:
    """Считывает TLS параметры и credentials тестовых владельцев."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--smtp-port", type=int, default=2158)
    parser.add_argument("--imap-port", type=int, default=2146)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    parser.add_argument("--alice-password", default=os.environ.get("LITTERKEEP_ALICE_PASSWORD"))
    parser.add_argument("--bob-password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD"))
    parsed = parser.parse_args()
    if parsed.alice_password is None or parsed.bob_password is None:
        raise ValueError("bootstrap Alice and Bob passwords must be supplied through environment")
    return parsed


def fixture(token: str) -> tuple[bytes, bytes, bytes]:
    """Строит вложенные comments вокруг Base64 CTE и возвращает ожидаемые octets."""
    payload = b"CTE comment parser " + token.encode("ascii") + b" exact payload"
    encoded = base64.b64encode(payload)
    raw = (
        b"From: alice@litterkeep.test\r\n"
        b"To: bob@litterkeep.test\r\n"
        b"Subject: CTE comments "
        + token.encode("ascii")
        + b"\r\nContent-Type: application/octet-stream\r\n"
        + b"Content-Transfer-Encoding: "
        + b"(outer (nested escaped \\) right-paren) still outer) "
        + b"base64 (trailing comment)\r\n\r\n"
        + encoded
        + b"\r\n"
    )
    return raw, payload, token.encode("ascii")


def deliver(settings: argparse.Namespace, raw: bytes) -> None:
    """Передаёт исходное письмо через аутентифицированный SMTP STARTTLS."""
    context = ssl.create_default_context(cafile=settings.ca)
    with smtplib.SMTP("127.0.0.1", settings.smtp_port, timeout=10) as client:
        client.ehlo()
        client.starttls(context=context)
        client.ehlo()
        client.login("alice", settings.alice_password)
        client.sendmail("alice@litterkeep.test", "bob@litterkeep.test", raw)


def literal_payload(records: list[object]) -> bytes:
    """Достаёт literal из FETCH response, не полагаясь на list position."""
    values = [
        value
        for record in records
        if isinstance(record, tuple)
        for value in reversed(record)
        if isinstance(value, bytes)
    ]
    return next(iter(values), b"")


def read_message(settings: argparse.Namespace, token: bytes) -> tuple[bytes, bytes, bytes]:
    """Возвращает BODYSTRUCTURE, decoded BINARY и оригинал доставленного RFC822."""
    context = ssl.create_default_context(cafile=settings.ca)
    with imaplib.IMAP4("127.0.0.1", settings.imap_port, timeout=10) as client:
        client.starttls(ssl_context=context)
        require_ok(client.login("bob", settings.bob_password), "LOGIN")
        require_ok(client.select("INBOX", readonly=True), "SELECT INBOX")
        search = require_ok(
            client.uid("SEARCH", None, "HEADER", "SUBJECT", token.decode("ascii")),
            "UID SEARCH by fixture token",
        )
        identifiers = b" ".join(item for item in search if isinstance(item, bytes)).split()
        if len(identifiers) != 1:
            raise AssertionError(f"expected one delivered message, got {identifiers!r}")
        identifier = next(iter(identifiers), b"")
        if not identifier:
            raise AssertionError("UID SEARCH returned no identifier")
        structure = require_ok(
            client.uid("FETCH", identifier, "(BODYSTRUCTURE)"),
            "BODYSTRUCTURE FETCH",
        )
        binary = require_ok(
            client.uid("FETCH", identifier, "(BINARY.PEEK[1])"),
            "BINARY.PEEK",
        )
        source = require_ok(client.uid("FETCH", identifier, "(RFC822)"), "RFC822 FETCH")
    structure_bytes = b" ".join(item for item in structure if isinstance(item, bytes))
    source_bytes = literal_payload(source)
    return structure_bytes, literal_payload(binary), source_bytes


def main() -> None:
    """Выполняет round-trip комментариев CTE через независимые TLS клиенты."""
    settings = options()
    raw, expected, token = fixture(uuid.uuid4().hex)
    deliver(settings, raw)
    structure, decoded, source = read_message(settings, token)
    if b'"BASE64"' not in structure:
        raise AssertionError(f"BODYSTRUCTURE did not recognize commented Base64 CTE: {structure!r}")
    if decoded != expected:
        raise AssertionError(f"commented Base64 did not decode exactly: {decoded!r}")
    if not source.endswith(raw):
        raise AssertionError("SMTP RFC822 octets were rewritten during mailbox storage")
    print("PASS mime-cte-comments: SMTP delivery and IMAP BINARY.PEEK")


if __name__ == "__main__":
    main()
