"""Проверяет RFC partial offsets после отбора IMAP header fields."""

import argparse
import csv
from enum import Enum
import imaplib
import ssl
import uuid


class ImapStatus(Enum):
    """Допустимые tagged completion для операций acceptance."""

    OK = "OK"


def require_ok(response: tuple[str, object], operation: str) -> object:
    """Требует успешный tagged response и сохраняет детали при ошибке."""
    status, detail = response
    try:
        observed = ImapStatus(status)
    except ValueError as error:
        raise AssertionError(f"{operation}: {status!r} {detail!r}") from error
    if observed is not ImapStatus.OK:
        raise AssertionError(f"{operation}: {status!r} {detail!r}")
    return detail


def bob_password(path: str) -> str:
    """Читает локальную тестовую учётную запись без публикации пароля."""
    with open(path, encoding="utf-8-sig", newline="") as source:
        for account, secret, *_ in csv.reader(source, delimiter="\t"):
            if account == "bob":
                return secret
    raise ValueError("Bob acceptance account is missing")


def literal(records: object, operation: str) -> bytes:
    """Извлекает literal по типу ответа, не полагаясь на список атрибутов."""
    if not isinstance(records, list):
        raise AssertionError(f"{operation}: invalid response {records!r}")
    for record in records:
        match record:
            case (_metadata, payload) if isinstance(payload, bytes):
                return payload
            case _:
                continue
    raise AssertionError(f"{operation}: literal is absent")


def inclusion_fixture(subject: str) -> bytes:
    """Ставит невыбранное поле перед Subject для inclusion partial."""
    return (
        "X-Ignored: before selected field\r\n"
        f"Subject: {subject}\r\n"
        "From: bob@example.test\r\n"
        "To: bob@example.test\r\n"
        "\r\n"
        "partial-header fixture\r\n"
    ).encode("ascii")


def exclusion_fixture(subject: str) -> bytes:
    """Ставит исключённый Subject перед оставшимися полями для NOT partial."""
    return (
        f"Subject: {subject}\r\n"
        "From: bob@example.test\r\n"
        "X-Ignored: later field\r\n"
        "To: bob@example.test\r\n"
        "\r\n"
        "partial-header fixture\r\n"
    ).encode("ascii")


def find_number(client: imaplib.IMAP4, subject: str) -> str:
    """Ищет ровно одно сообщение по уникальному acceptance subject."""
    found = require_ok(
        client.search(None, "HEADER", "SUBJECT", f'"{subject}"'), "SEARCH"
    )
    if not isinstance(found, list):
        raise AssertionError(f"invalid SEARCH response: {found!r}")
    numbers = b" ".join(item for item in found if isinstance(item, bytes)).split()
    match numbers:
        case [number_bytes]:
            return number_bytes.decode("ascii")
        case _:
            raise AssertionError(f"expected one message, found {numbers!r}")


def verify_subset(
    client: imaplib.IMAP4,
    number: str,
    selector: str,
    expected: bytes,
    operation: str,
) -> None:
    """Сверяет exact partial bytes и response origin label."""
    item = f"BODY.PEEK[{selector}]<0.{len(expected)}>"
    response = require_ok(client.fetch(number, f"({item})"), operation)
    actual = literal(response, operation)
    if actual != expected:
        raise AssertionError(f"{operation}: expected {expected!r}, got {actual!r}")
    fields = []
    for record in response:
        match record:
            case (field, _payload) if isinstance(field, bytes):
                fields.append(field)
            case raw if isinstance(raw, bytes):
                fields.append(raw)
            case _:
                continue
    metadata = b" ".join(fields)
    origin = f"BODY[{selector}]<0>".encode("ascii")
    if origin not in metadata:
        raise AssertionError(f"{operation}: missing origin label in {metadata!r}")


def run(settings: argparse.Namespace) -> None:
    """APPEND-ит fixture и проверяет inclusion/exclusion partial selectors."""
    inclusion_subject = f"header inclusion {uuid.uuid4().hex}"
    exclusion_subject = f"header exclusion {uuid.uuid4().hex}"
    selected_message = inclusion_fixture(inclusion_subject)
    excluded_message = exclusion_fixture(exclusion_subject)
    selected = f"Subject: {inclusion_subject}\r\n\r\n".encode("ascii")
    excluded = (
        b"From: bob@example.test\r\n"
        b"X-Ignored: later field\r\n"
        b"To: bob@example.test\r\n"
        b"\r\n"
    )
    context = ssl.create_default_context(cafile=settings.ca)
    context.check_hostname = False
    with imaplib.IMAP4(settings.host, settings.port, timeout=10) as client:
        client.starttls(ssl_context=context)
        require_ok(client.login("bob", bob_password(settings.credentials)), "LOGIN")
        require_ok(
            client.append("INBOX", None, None, selected_message), "APPEND inclusion"
        )
        require_ok(
            client.append("INBOX", None, None, excluded_message), "APPEND exclusion"
        )
        require_ok(client.select("INBOX"), "SELECT writable INBOX")
        inclusion_number = find_number(client, inclusion_subject)
        exclusion_number = find_number(client, exclusion_subject)
        verify_subset(
            client,
            inclusion_number,
            "HEADER.FIELDS (subject)",
            selected[:8],
            "HEADER.FIELDS partial",
        )
        verify_subset(
            client,
            exclusion_number,
            "HEADER.FIELDS.NOT (subject)",
            excluded[:8],
            "HEADER.FIELDS.NOT partial",
        )
        require_ok(client.noop(), "NOOP after partial FETCH")


def main() -> None:
    """Разбирает STARTTLS endpoint и локальный credentials fixture."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", required=True)
    run(parser.parse_args())
    print("PASS IMAP header subset partial offsets")


if __name__ == "__main__":
    main()
