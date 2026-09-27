"""Проверяет RFC 2046 multipart/digest через независимый IMAP-клиент."""

import argparse
from enum import Enum
import imaplib
import os
import ssl
import uuid


class ImapStatus(Enum):
    """Допустимый статус успешной команды в сценарии."""

    OK = "OK"


class BodyStructurePart(Enum):
    """MIME-типы, обязательные для headerless digest entity."""

    DIGEST = b'"DIGEST"'
    MESSAGE = b'"MESSAGE" "RFC822"'
    DEFAULT_BODY = b'"TEXT" "PLAIN"'


def require_ok(response: tuple[str, object], operation: str) -> object:
    """Преобразует статус wire в Enum и прекращает тест при отказе."""
    status, detail = response
    try:
        observed = ImapStatus(status)
    except ValueError as error:
        raise AssertionError(f"{operation}: {status!r} {detail!r}") from error
    if observed is not ImapStatus.OK:
        raise AssertionError(f"{operation}: {status!r} {detail!r}")
    return detail


def message_fixture() -> tuple[bytes, bytes]:
    """Создаёт RFC822 с headerless multipart/digest body-part."""
    token = uuid.uuid4().hex
    boundary = f"digest-{token}"
    subject = f"digest acceptance {token}"
    message = (
        f"Subject: {subject}\r\n"
        f"Content-Type: multipart/digest; boundary={boundary}\r\n"
        "\r\n"
        f"--{boundary}\r\n"
        "\r\n"
        "Subject: nested message\r\n"
        "\r\n"
        "message body\r\n"
        f"--{boundary}--\r\n"
    ).encode("ascii")
    return message, subject.encode("ascii")


def literal(records: object, operation: str) -> bytes:
    """Извлекает FETCH literal без зависимости от порядка data items."""
    if not isinstance(records, list):
        raise AssertionError(f"{operation}: unexpected records {records!r}")
    for record in records:
        match record:
            case (_metadata, payload) if isinstance(payload, bytes):
                return payload
            case _:
                continue
    raise AssertionError(f"{operation}: response literal is absent")


def find_message(client: imaplib.IMAP4, subject: bytes) -> str:
    """Находит только письмо этого запуска по уникальному Subject."""
    search = require_ok(
        client.search(None, "HEADER", "SUBJECT", f'"{subject.decode("ascii")}"'),
        "SEARCH digest subject",
    )
    if not isinstance(search, list):
        raise AssertionError(f"SEARCH returned invalid data: {search!r}")
    identifiers = b" ".join(
        value for value in search if isinstance(value, bytes)
    ).decode("ascii").split()
    match identifiers:
        case [message_number]:
            return message_number
        case _:
            raise AssertionError(f"expected one digest message, got {identifiers!r}")


def verify_digest_structure(client: imaplib.IMAP4, number: str) -> None:
    """Проверяет wire BODYSTRUCTURE, а не только MIME parser tree."""
    response = require_ok(
        client.fetch(number, "(BODYSTRUCTURE)"), "FETCH BODYSTRUCTURE"
    )
    if not isinstance(response, list):
        raise AssertionError(f"BODYSTRUCTURE returned invalid data: {response!r}")
    structure = b" ".join(value for value in response if isinstance(value, bytes))
    for expected in BodyStructurePart:
        if structure.count(expected.value) != 1:
            raise AssertionError(
                f"BODYSTRUCTURE {expected.name} mismatch: {structure!r}"
            )


def verify_source(client: imaplib.IMAP4, number: str, expected: bytes) -> None:
    """Подтверждает, что fallback не переписал MIME/RFC822 octets."""
    response = require_ok(client.fetch(number, "(RFC822)"), "FETCH RFC822")
    observed = literal(response, "FETCH RFC822")
    if observed != expected:
        raise AssertionError("RFC822 bytes changed while projecting digest default")


def verify_encapsulated_sections(client: imaplib.IMAP4, number: str) -> None:
    """Проверяет IMAP body-part selectors для вложенного RFC822 сообщения."""
    sections = (
        ("BODY.PEEK[1]", b"Subject: nested message\r\n\r\nmessage body"),
        ("BODY.PEEK[1.HEADER]", b"Subject: nested message\r\n\r\n"),
        ("BODY.PEEK[1.TEXT]", b"message body"),
        ("BODY.PEEK[1.1]", b"message body"),
    )
    for selector, expected in sections:
        response = require_ok(
            client.fetch(number, f"({selector})"), f"FETCH {selector}"
        )
        observed = literal(response, selector)
        if observed != expected:
            raise AssertionError(
                f"{selector} mismatch: expected {expected!r}, got {observed!r}"
            )


def options() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="localhost")
    parser.add_argument("--imap-port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD"))
    parsed = parser.parse_args()
    if parsed.password is None:
        raise ValueError("provide --password or LITTERKEEP_BOB_PASSWORD")
    return parsed


def main() -> None:
    selected = options()
    source, subject = message_fixture()
    context = ssl.create_default_context(cafile=selected.ca)
    context.check_hostname = False
    with imaplib.IMAP4(selected.host, selected.imap_port, timeout=10) as client:
        client.starttls(ssl_context=context)
        require_ok(client.login("bob", selected.password), "LOGIN")
        require_ok(client.append("INBOX", None, None, source), "APPEND")
        require_ok(client.select("INBOX", readonly=True), "SELECT INBOX")
        number = find_message(client, subject)
        verify_digest_structure(client, number)
        verify_encapsulated_sections(client, number)
        verify_source(client, number, source)
    print("PASS RFC 2046 multipart/digest default and exact RFC822 source")


if __name__ == "__main__":
    main()
