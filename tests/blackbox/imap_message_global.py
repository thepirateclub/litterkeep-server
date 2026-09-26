"""Проверяет вложенный message/global по независимому IMAP wire-клиенту."""

import argparse
import base64
import enum
import imaplib
import os
import ssl
import uuid


class ReplyStatus(enum.Enum):
    """Допустимые верхнеуровневые исходы команд независимого IMAP клиента."""

    OK = "OK"


def require_ok(response: tuple[str, list[object]], operation: str) -> list[object]:
    """Прерывает acceptance на первом неуспешном tagged-ответе сервера."""
    status, records = response
    if ReplyStatus(status) is not ReplyStatus.OK:
        raise AssertionError(f"{operation}: {status} {records!r}")
    return records


def enable_utf8_accept(client: imaplib.IMAP4) -> None:
    """Включает RFC 9755 режим, не рекламируемый до общего profile gate."""
    status, _ = client._simple_command("ENABLE", "UTF8=ACCEPT")
    response_type, enabled = client.response("ENABLED")
    if status != "OK" or response_type != "ENABLED" or not any(
        b"UTF8=ACCEPT" in item for item in enabled
    ):
        raise AssertionError(f"ENABLE UTF8=ACCEPT failed: {status!r} {enabled!r}")
    client._encoding = "utf-8"


def response_octets(value: object) -> bytes:
    """Собирает metadata из вложенных структур imaplib без позиционных индексов."""
    match value:
        case bytes():
            return value
        case tuple() | list():
            return b" ".join(response_octets(part) for part in value)
        case _:
            return b""


def literal_octets(records: list[object], operation: str) -> bytes:
    """Извлекает ровно literal FETCH, не принимая metadata за payload."""
    for record in records:
        match record:
            case (_metadata, payload) if isinstance(payload, bytes):
                return payload
            case _:
                continue
    raise AssertionError(f"{operation} omitted its literal: {records!r}")


def nested_message() -> bytes:
    """Строит UTF-8 message/global с вложенной 8bit MIME text частью."""
    return (
        "Subject: Привет\r\n"
        "Content-Type: text/plain; charset=utf-8\r\n"
        "Content-Transfer-Encoding: 8bit\r\n"
        "\r\n"
        "Тело вложенного письма ✓\r\n"
    ).encode("utf-8")


def run_acceptance(host: str, port: int, ca: str, password: str) -> None:
    """Сверяет encoded, decoded и исходные byte ranges на реальном IMAP socket."""
    context = ssl.create_default_context(cafile=ca)
    mailbox = f"MessageGlobal-{uuid.uuid4().hex}"
    inner = nested_message()
    encoded_inner = base64.b64encode(inner)
    raw = (
        b"From: alice@example.test\r\n"
        b"To: bob@example.test\r\n"
        b"Content-Type: message/global\r\n"
        b"Content-Transfer-Encoding: base64\r\n\r\n"
        + encoded_inner
    )
    with imaplib.IMAP4(host, port, timeout=10) as client:
        client.starttls(ssl_context=context)
        require_ok(client.login("bob", password), "LOGIN")
        enable_utf8_accept(client)
        require_ok(client.create(mailbox), "CREATE")
        require_ok(client.append(mailbox, None, None, raw), "APPEND")
        require_ok(client.select(mailbox, readonly=True), "SELECT")

        structure_records = require_ok(
            client.fetch("1", "(BODYSTRUCTURE)"), "BODYSTRUCTURE FETCH"
        )
        structure = response_octets(structure_records)
        wire_part = literal_octets(
            require_ok(client.fetch("1", "(BODY[1])"), "encoded BODY[1]"),
            "BODY[1]",
        )
        nested_body = literal_octets(
            require_ok(client.fetch("1", "(BODY[1.1])"), "nested BODY[1.1]"),
            "BODY[1.1]",
        )
        nested_header = literal_octets(
            require_ok(
                client.fetch("1", "(BODY[1.HEADER])"), "nested BODY[1.HEADER]"
            ),
            "BODY[1.HEADER]",
        )
        binary_body = literal_octets(
            require_ok(client.fetch("1", "(BINARY[1.1])"), "nested BINARY[1.1]"),
            "BINARY[1.1]",
        )
        retained = literal_octets(
            require_ok(client.fetch("1", "(RFC822)"), "RFC822 FETCH"),
            "RFC822",
        )

    for required in (b'"MESSAGE" "GLOBAL"', b'"TEXT" "PLAIN"'):
        if required not in structure:
            raise AssertionError(f"BODYSTRUCTURE lacks {required!r}: {structure!r}")
    if wire_part != encoded_inner:
        raise AssertionError("BODY[1] did not preserve the encoded MIME body")
    if nested_body != "Тело вложенного письма ✓\r\n".encode("utf-8"):
        raise AssertionError(f"nested BODY range changed: {nested_body!r}")
    expected_header = (
        "Subject: Привет\r\n"
        "Content-Type: text/plain; charset=utf-8\r\n"
        "Content-Transfer-Encoding: 8bit\r\n\r\n"
    ).encode("utf-8")
    if nested_header != expected_header:
        raise AssertionError(f"nested header range changed: {nested_header!r}")
    if binary_body != nested_body:
        raise AssertionError("BINARY did not expose decoded nested body octets")
    if retained != raw:
        raise AssertionError("RFC822 FETCH changed original message octets")


def main() -> None:
    """Подготавливает параметры независимого клиента из CLI и окружения."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=2144)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    parser.add_argument("--password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD"))
    options = parser.parse_args()
    if options.password is None:
        raise ValueError("provide --password or LITTERKEEP_BOB_PASSWORD")
    run_acceptance(options.host, options.port, options.ca, options.password)
    print(
        "PASS imap-message-global with ENABLE UTF8=ACCEPT: "
        "BODYSTRUCTURE, encoded/decoded ranges, RFC822"
    )


if __name__ == "__main__":
    main()
