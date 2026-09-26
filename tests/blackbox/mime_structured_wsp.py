"""Проверяет ASCII WSP в MIME structured values через независимый IMAP peer."""

import argparse
import enum
import imaplib
import os
import ssl
import uuid


class ImapStatus(enum.Enum):
    """Единственный успешный tagged outcome сетевой проверки."""

    OK = "OK"


def require_ok(response: tuple[str, list[object]], operation: str) -> list[object]:
    """Останавливает сценарий при первом неуспешном IMAP статусе."""
    status, records = response
    if ImapStatus(status) is not ImapStatus.OK:
        raise AssertionError(f"{operation}: {status!r} {records!r}")
    return records


def literal(records: list[object], operation: str) -> bytes:
    """Извлекает literal без смешивания его с FETCH metadata."""
    for record in records:
        match record:
            case (_metadata, payload) if isinstance(payload, bytes):
                return payload
            case _:
                continue
    raise AssertionError(f"{operation} did not return a literal")


def fixture_messages() -> tuple[bytes, ...]:
    """Формирует structured WSP и unstructured UTF-8 regressions."""
    subject = uuid.uuid4().hex.encode("ascii")
    prefix = b"From: alice@example.test\r\nTo: bob@example.test\r\nSubject: "
    invalid_type = (
        prefix + b"invalid-" + subject
        + b"\r\nContent-Type: \xc2\xa0application/octet-stream; name=x\r\n\r\nbody\r\n"
    )
    quoted_data = (
        prefix + b"quoted-" + subject
        + b'\r\nContent-Type: application/octet-stream; name="\xc2\xa0"\r\n\r\nbody\r\n'
    )
    invalid_disposition = (
        prefix + b"disposition-" + subject
        + b"\r\nContent-Type: text/plain\r\n"
        + b"Content-Disposition: \xc2\xa0attachment; filename=x\r\n\r\nbody\r\n"
    )
    invalid_language = (
        prefix + b"language-" + subject
        + b"\r\nContent-Language: \xc2\xa0en\r\n\r\nbody\r\n"
    )
    invalid_location = (
        prefix + b"location-" + subject
        + b"\r\nContent-Location: \xc2\xa0https://example.test/x\r\n\r\nbody\r\n"
    )
    description_data = (
        prefix + b"description-" + subject
        + b"\r\nContent-Description: \xc2\xa0note\xc2\xa0\r\n\r\nbody\r\n"
    )
    return (
        invalid_type, quoted_data, invalid_disposition,
        invalid_language, invalid_location, description_data,
    )


def inspect_mailbox(
    options: argparse.Namespace, expected_sources: tuple[bytes, ...]
) -> list[tuple[bytes, bytes]]:
    """Добавляет fixtures и возвращает независимые BODYSTRUCTURE/RFC822 пары."""
    context = ssl.create_default_context(cafile=options.ca)
    mailbox = f"MimeWsp-{uuid.uuid4().hex}"
    result = []
    with imaplib.IMAP4(options.host, options.port, timeout=10) as client:
        client.starttls(ssl_context=context)
        require_ok(client.login("bob", options.password), "LOGIN")
        require_ok(client.create(mailbox), "CREATE")
        for source in expected_sources:
            require_ok(client.append(mailbox, None, None, source), "APPEND")
        require_ok(client.select(mailbox, readonly=True), "SELECT")
        search = require_ok(client.uid("SEARCH", None, "ALL"), "UID SEARCH")
        identifiers = b" ".join(
            record for record in search if isinstance(record, bytes)
        ).split()
        if len(identifiers) != len(expected_sources):
            raise AssertionError(f"unexpected fixture count: {identifiers!r}")
        for identifier in identifiers:
            structure = require_ok(
                client.uid("FETCH", identifier, "(BODYSTRUCTURE)"),
                "BODYSTRUCTURE FETCH",
            )
            source = require_ok(
                client.uid("FETCH", identifier, "(RFC822)"), "RFC822 FETCH"
            )
            structure_bytes = b" ".join(
                record for record in structure if isinstance(record, bytes)
            )
            result.append((structure_bytes, literal(source, "RFC822 FETCH")))
    return result


def verify(records: list[tuple[bytes, bytes]], expected: tuple[bytes, ...]) -> None:
    """Подтверждает fallback и точное сохранение оригинальных MIME bytes."""
    structures = {source: structure for structure, source in records}
    if set(structures) != set(expected):
        raise AssertionError("IMAP changed or omitted an original RFC822 literal")
    invalid_type = next(source for source in expected if b"invalid-" in source)
    fallback = structures[invalid_type]
    if b'"TEXT" "PLAIN"' not in fallback:
        raise AssertionError(f"invalid Content-Type did not use fallback: {fallback!r}")
    if b'"APPLICATION" "OCTET-STREAM"' in fallback or b'"NAME"' in fallback:
        raise AssertionError(f"NBSP was incorrectly trimmed from Content-Type: {fallback!r}")
    quoted_data = next(source for source in expected if b"quoted-" in source)
    if b'"NAME" "\xc2\xa0"' not in structures[quoted_data]:
        raise AssertionError("quoted NBSP was not preserved as a parameter value")
    invalid_disposition = next(
        source for source in expected if b"disposition-" in source
    )
    if b'"attachment"' in structures[invalid_disposition]:
        raise AssertionError("NBSP was incorrectly trimmed from disposition token")
    invalid_language = next(source for source in expected if b"language-" in source)
    if b'"en"' in structures[invalid_language]:
        raise AssertionError("NBSP was incorrectly treated as Content-Language CFWS")
    invalid_location = next(source for source in expected if b"location-" in source)
    if b"https://example.test/x" in structures[invalid_location]:
        raise AssertionError("NBSP was incorrectly trimmed from Content-Location")
    description = next(source for source in expected if b"description-" in source)
    if b'"\xc2\xa0note\xc2\xa0"' not in structures[description]:
        raise AssertionError("Content-Description Unicode whitespace was not preserved")


def main() -> None:
    """Запускает acceptance с независимым Python стандартным IMAP клиентом."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=2148)
    parser.add_argument("--ca", default="data/g17-mime-ascii-wsp/pki/ca.pem")
    parser.add_argument(
        "--password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD")
    )
    options = parser.parse_args()
    if options.password is None:
        raise ValueError("provide --password or LITTERKEEP_BOB_PASSWORD")
    expected = fixture_messages()
    verify(inspect_mailbox(options, expected), expected)
    print("PASS MIME WSP IMAP wire profile: fallback, quoted value and raw bytes")


if __name__ == "__main__":
    main()
