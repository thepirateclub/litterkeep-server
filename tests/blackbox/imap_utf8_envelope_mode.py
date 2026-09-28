"""Проверяет FETCH ENVELOPE до и после ENABLE UTF8=ACCEPT."""

import argparse
import imaplib
import ssl
import uuid
from pathlib import Path


def require(result: tuple[str, object], operation: str) -> object:
    """Останавливает тест при любом неуспешном tagged response."""
    status, data = result
    if status != "OK":
        raise AssertionError(f"{operation}: {status} {data!r}")
    return data


def open_client(host: str, port: int, ca: str, password: str) -> imaplib.IMAP4:
    """Устанавливает независимое IMAP соединение с проверкой сертификата."""
    context = ssl.create_default_context(cafile=ca)
    client = imaplib.IMAP4(host, port, timeout=10)
    client.starttls(ssl_context=context)
    require(client.login("bob", password), "LOGIN")
    return client


def bob_password(path: Path) -> str:
    """Загружает локальный acceptance secret, не показывая его в выводе."""
    for line in path.read_text(encoding="utf-8").splitlines():
        identity, separator, password = line.partition("\t")
        match (bool(separator), identity == "bob", bool(password)):
            case (True, True, True):
                return password
    raise AssertionError("bob acceptance credential is missing")


def response_bytes(data: object) -> bytes:
    """Собирает поля и literal chunks из FETCH ответа imaplib."""
    match data:
        case bytes():
            return data
        case tuple() | list():
            return b"".join(response_bytes(part) for part in data)
        case _:
            return b""


def fetch_envelope(client: imaplib.IMAP4, mailbox: str) -> bytes:
    """Выбирает ящик и возвращает ENVELOPE вместе с разобранными literals."""
    require(client.select(mailbox, readonly=True), "SELECT")
    return response_bytes(require(client.fetch("1", "(ENVELOPE)"), "FETCH ENVELOPE"))


def main() -> None:
    """Сверяет legacy literals и UTF8=ACCEPT quoted strings на одном письме."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    options = parser.parse_args()
    mailbox = f"EnvelopeUtf8-{uuid.uuid4().hex}"
    password = bob_password(options.credentials)
    subject = "Привет".encode("utf-8")
    message = (
        b"From: =?UTF-8?B?0J/RgNC40LLQtdGC?= <alice@example.test>\r\n"
        b"To: bob@example.test\r\n"
        b"Subject: =?UTF-8?B?0J/RgNC40LLQtdGC?=\r\n"
        b"Content-Type: application/pdf; name*=UTF-8''caf%C3%A9.pdf\r\n"
        b"Content-Disposition: attachment; filename*=UTF-8''caf%C3%A9.pdf\r\n"
        b"Content-Description: =?UTF-8?B?Y2Fmw6k=?=\r\n"
        b"Content-Transfer-Encoding: base64\r\n\r\nUERG\r\n"
    )

    legacy = open_client(options.host, options.port, options.ca, password)
    require(legacy.create(mailbox), "CREATE")
    require(legacy.append(mailbox, None, None, message), "APPEND")
    legacy_wire = fetch_envelope(legacy, mailbox)
    match (subject in legacy_wire, b"{12}" in legacy_wire):
        case (True, True):
            pass
        case _:
            raise AssertionError(f"legacy ENVELOPE did not use UTF-8 literals: {legacy_wire!r}")
    if b'"' + subject + b'"' in legacy_wire:
        raise AssertionError(f"legacy quoted-string contained UTF-8: {legacy_wire!r}")
    legacy_structure = response_bytes(
        require(legacy.fetch("1", "(BODYSTRUCTURE)"), "FETCH BODYSTRUCTURE")
    )
    match (b"{5}" in legacy_structure, b"{9}" in legacy_structure):
        case (True, True):
            pass
        case _:
            raise AssertionError(
                f"legacy BODYSTRUCTURE omitted MIME literals: {legacy_structure!r}"
            )
    if b'"caf\xc3\xa9"' in legacy_structure:
        raise AssertionError(f"legacy BODYSTRUCTURE string mode failed: {legacy_structure!r}")
    if "café".encode("utf-8") not in legacy_structure:
        raise AssertionError(f"legacy BODYSTRUCTURE lost decoded MIME text: {legacy_structure!r}")
    legacy.logout()

    enabled = open_client(options.host, options.port, options.ca, password)
    status, _ = enabled._simple_command("ENABLE", "UTF8=ACCEPT")
    response_type, values = enabled.response("ENABLED")
    enabled_marker = any(b"UTF8=ACCEPT" in item for item in values)
    match (status, response_type, enabled_marker):
        case ("OK", "ENABLED", True):
            pass
        case _:
            raise AssertionError(f"ENABLE failed: {status!r} {values!r}")
    enabled._encoding = "utf-8"
    utf8_wire = fetch_envelope(enabled, mailbox)
    if b'"' + subject + b'"' not in utf8_wire:
        raise AssertionError(f"enabled ENVELOPE did not use quoted UTF-8: {utf8_wire!r}")
    enabled_structure = response_bytes(
        require(enabled.fetch("1", "(BODYSTRUCTURE)"), "FETCH BODYSTRUCTURE")
    )
    if b'"caf\xc3\xa9"' not in enabled_structure:
        raise AssertionError(f"enabled BODYSTRUCTURE did not quote UTF-8: {enabled_structure!r}")
    if b'"caf\xc3\xa9.pdf"' not in enabled_structure:
        raise AssertionError(f"enabled MIME parameter did not quote UTF-8: {enabled_structure!r}")
    enabled.logout()
    print("PASS imap-utf8-envelope-mode: legacy literals and enabled quoted UTF-8")


if __name__ == "__main__":
    main()
