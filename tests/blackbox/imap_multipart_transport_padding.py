"""Проверяет MIME multipart boundary padding через независимый IMAP-клиент."""

import argparse
import base64
from enum import Enum
import imaplib
import os
import ssl
import uuid


class ImapStatus(Enum):
    """Успешное состояние независимых IMAP-команд."""

    OK = "OK"


class BodyStructureToken(Enum):
    """Типизированные MIME-узлы, ожидаемые в BODYSTRUCTURE."""

    MIXED = b'"MIXED"'
    TEXT_PLAIN = b'"TEXT" "PLAIN"'
    APPLICATION_OCTETS = b'"APPLICATION" "OCTET-STREAM"'


def require_ok(response: tuple[str, object], operation: str) -> object:
    """Останавливает acceptance при любом неожиданном IMAP status."""
    status, detail = response
    try:
        observed = ImapStatus(status)
    except ValueError as error:
        raise AssertionError(f"{operation}: {status!r} {detail!r}") from error
    if observed is not ImapStatus.OK:
        raise AssertionError(f"{operation}: {status!r} {detail!r}")
    return detail


def options() -> argparse.Namespace:
    """Читает только адрес listener-а и локальные данные TLS/аккаунта."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="localhost")
    parser.add_argument("--imap-port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument(
        "--password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD")
    )
    return parser.parse_args()


def fixture() -> tuple[bytes, bytes, bytes, str]:
    """Строит multipart с transport-padding и boundary-похожим текстом."""
    boundary = ("g17-" + uuid.uuid4().hex).encode("ascii")
    subject = "multipart padding " + uuid.uuid4().hex
    attachment = bytes(range(256)) + b"\x00padding\xff"
    encoded = base64.encodebytes(attachment).replace(b"\n", b"\r\n")
    raw = (
        b"From: alice@test\r\nTo: bob@test\r\nSubject: "
        + subject.encode("ascii")
        + b"\r\nContent-Type: multipart/mixed; boundary=\""
        + boundary
        + b"\"\r\n\r\nexplanatory preamble\r\n--"
        + boundary
        + b" \t\r\nContent-Type: text/plain; charset=utf-8\r\n"
        + b"Content-Transfer-Encoding: 7bit\r\n\r\nfirst line\r\nlast line\r\n--"
        + boundary
        + b"\t\r\nContent-Type: application/octet-stream\r\n"
        + b"Content-Transfer-Encoding: base64\r\n\r\n"
        + encoded
        + b"--"
        + boundary
        + b"-- \t\r\nepilogue\r\n--"
        + boundary
        + b"\r\nContent-Type: text/html\r\n\r\nnot a third part\r\n"
    )
    expected_text = b"first line\r\nlast line"
    return raw, expected_text, attachment, subject


def literal(records: list[object]) -> bytes:
    """Извлекает literal FETCH без предположений о его позиции."""
    for record in records:
        match record:
            case (_metadata, payload) if isinstance(payload, bytes):
                return payload
            case _:
                continue
    raise AssertionError(f"FETCH literal is absent: {records!r}")


def main() -> None:
    selected = options()
    if selected.password is None:
        raise ValueError("provide --password or LITTERKEEP_BOB_PASSWORD")
    raw, expected_text, expected_attachment, subject = fixture()

    context = ssl.create_default_context(cafile=selected.ca)
    context.check_hostname = False
    with imaplib.IMAP4(selected.host, selected.imap_port, timeout=10) as client:
        client.starttls(ssl_context=context)
        require_ok(client.login("bob", selected.password), "LOGIN")
        require_ok(client.append("INBOX", None, None, raw), "APPEND")
        require_ok(client.select("INBOX", readonly=True), "SELECT INBOX")
        matches = require_ok(
            client.search(None, "HEADER", "SUBJECT", f'"{subject}"'),
            "SEARCH SUBJECT",
        )
        identifiers = b" ".join(
            item for item in matches if isinstance(item, bytes)
        ).decode("ascii").split()
        match identifiers:
            case [message_number]:
                pass
            case _:
                raise AssertionError(f"message search returned {identifiers!r}")

        structure = require_ok(
            client.fetch(message_number, "(BODYSTRUCTURE)"), "FETCH BODYSTRUCTURE"
        )
        wire_structure = b" ".join(
            item for item in structure if isinstance(item, bytes)
        )
        for token in BodyStructureToken:
            if wire_structure.count(token.value) != 1:
                raise AssertionError(
                    f"BODYSTRUCTURE token {token.name} count mismatch: {wire_structure!r}"
                )
        first_part = require_ok(
            client.fetch(message_number, "(BODY.PEEK[1])"), "FETCH first part"
        )
        if literal(first_part) != expected_text:
            raise AssertionError("boundary padding shifted or truncated the first part")
        binary_part = require_ok(
            client.fetch(message_number, "(BINARY.PEEK[2])"), "FETCH attachment"
        )
        if literal(binary_part) != expected_attachment:
            raise AssertionError("padded closing boundary changed decoded attachment")
        full_message = require_ok(
            client.fetch(message_number, "(RFC822)"), "FETCH original message"
        )
        if literal(full_message) != raw:
            raise AssertionError("multipart source bytes, preamble or epilogue changed")

    print("PASS RFC 2046 multipart: padding, ignored epilogue delimiters and exact bytes")


if __name__ == "__main__":
    main()
