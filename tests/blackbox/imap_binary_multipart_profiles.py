"""Проверяет IMAP BINARY traversal через multipart subtype по wire."""

import argparse
from enum import Enum
import imaplib
import os
import ssl
import uuid


class ImapStatus(Enum):
    """Единственный допустимый успешный статус команд сценария."""

    OK = "OK"


class MultipartProfile(Enum):
    """Проверяемые MIME-контейнеры и вложенные message entity."""

    MIXED = "mixed"
    ALTERNATIVE = "alternative"
    RELATED = "related"
    DIGEST = "digest"
    GLOBAL = "global"


def require_ok(response: tuple[str, object], command: str) -> object:
    """Проверяет tagged status без неявного принятия неизвестного значения."""
    status, detail = response
    try:
        observed = ImapStatus(status)
    except ValueError as error:
        raise AssertionError(f"{command}: {status!r} {detail!r}") from error
    if observed is not ImapStatus.OK:
        raise AssertionError(f"{command}: {status!r} {detail!r}")
    return detail


def encapsulated_message() -> str:
    """Создаёт сообщение с QP leaf для проверки реального CTE decode."""
    return (
        "Subject: nested binary profile\r\n"
        "Content-Type: text/plain; charset=utf-8\r\n"
        "Content-Transfer-Encoding: quoted-printable\r\n"
        "\r\n"
        "message=20body"
    )


def child_entity(profile: MultipartProfile) -> str:
    """Возвращает MIME child, не смешивая выбор типа с сериализацией wire."""
    nested = encapsulated_message()
    match profile:
        case MultipartProfile.DIGEST:
            return f"\r\n{nested}"
        case MultipartProfile.GLOBAL:
            return (
                "Content-Type: message/global\r\n\r\n"
                "Subject: nested binary profile\r\n"
                "Content-Type: text/plain; charset=utf-8\r\n\r\nmessage body"
            )
        case MultipartProfile.MIXED | MultipartProfile.ALTERNATIVE | MultipartProfile.RELATED:
            return f"Content-Type: message/rfc822\r\n\r\n{nested}"


def multipart_type(profile: MultipartProfile) -> str:
    """Кодирует разрешённый subtype и обязательный related root type."""
    match profile:
        case MultipartProfile.MIXED | MultipartProfile.GLOBAL:
            return "multipart/mixed"
        case MultipartProfile.ALTERNATIVE:
            return "multipart/alternative"
        case MultipartProfile.RELATED:
            return 'multipart/related; type="message/rfc822"'
        case MultipartProfile.DIGEST:
            return "multipart/digest"


def expected_subtype(profile: MultipartProfile) -> str:
    """Возвращает subtype контейнера, учитывая message/global child fixture."""
    match profile:
        case MultipartProfile.GLOBAL:
            return MultipartProfile.MIXED.value.upper()
        case _:
            return profile.value.upper()


def expected_nested_type(profile: MultipartProfile) -> bytes:
    """Возвращает RFC media subtype вложенного message entity."""
    match profile:
        case MultipartProfile.GLOBAL:
            return b'"GLOBAL"'
        case _:
            return b'"RFC822"'


def second_part(profile: MultipartProfile, boundary: str) -> str:
    """Добавляет обязательную вторую alternative-представление."""
    match profile:
        case MultipartProfile.ALTERNATIVE:
            return (
                f"--{boundary}\r\nContent-Type: text/plain\r\n\r\n"
                "fallback alternative\r\n"
            )
        case _:
            return ""


def message_fixture(profile: MultipartProfile) -> tuple[bytes, bytes]:
    """Формирует самостоятельное письмо с валидным контейнером и boundary."""
    token = uuid.uuid4().hex
    boundary = f"binary-{token}"
    subject = f"binary multipart {profile.name} {token}"
    first = child_entity(profile)
    second = second_part(profile, boundary)
    source = (
        f"Subject: {subject}\r\n"
        f"Content-Type: {multipart_type(profile)}; boundary={boundary}\r\n"
        "\r\n"
        f"--{boundary}\r\n{first}\r\n"
        f"{second}"
        f"--{boundary}--\r\n"
    ).encode("ascii")
    return source, subject.encode("ascii")


def literal(response: object, operation: str) -> bytes:
    """Извлекает literal по типу, не опираясь на позицию FETCH-элемента."""
    if not isinstance(response, list):
        raise AssertionError(f"{operation}: invalid response {response!r}")
    for record in response:
        match record:
            case (_metadata, payload) if isinstance(payload, bytes):
                return payload
            case _:
                continue
    raise AssertionError(f"{operation}: literal is absent")


def fetch_metadata(response: object, operation: str) -> bytes:
    """Собирает FETCH-имя из tuple и нетипизированных response records."""
    if not isinstance(response, list):
        raise AssertionError(f"{operation}: invalid response {response!r}")
    fields = []
    for record in response:
        match record:
            case (metadata, _payload) if isinstance(metadata, bytes):
                fields.append(metadata)
            case raw if isinstance(raw, bytes):
                fields.append(raw)
            case _:
                continue
    return b" ".join(fields)


def find_message(client: imaplib.IMAP4, subject: bytes) -> str:
    """Находит ровно письмо данного уникального profile run."""
    query = f'"{subject.decode("ascii")}"'
    result = require_ok(client.search(None, "HEADER", "SUBJECT", query), "SEARCH")
    if not isinstance(result, list):
        raise AssertionError(f"SEARCH returned invalid data: {result!r}")
    identifiers = b" ".join(value for value in result if isinstance(value, bytes))
    match identifiers.decode("ascii").split():
        case [number]:
            return number
        case found:
            raise AssertionError(f"expected one result, got {found!r}")


def verify_fetch(client: imaplib.IMAP4, number: str, profile: MultipartProfile) -> None:
    """Проверяет subtype проекцию и вложенный CTE-decoded leaf по wire."""
    structure = require_ok(client.fetch(number, "(BODYSTRUCTURE)"), "BODYSTRUCTURE")
    if not isinstance(structure, list):
        raise AssertionError(f"BODYSTRUCTURE returned invalid data: {structure!r}")
    encoded = b" ".join(value for value in structure if isinstance(value, bytes))
    subtype_token = f'"{expected_subtype(profile)}"'.encode("ascii")
    if subtype_token not in encoded:
        raise AssertionError(f"subtype projection mismatch: {encoded!r}")
    nested_type = expected_nested_type(profile)
    if nested_type not in encoded:
        raise AssertionError(f"embedded message type mismatch: {encoded!r}")

    data = require_ok(
        client.fetch(number, "(BINARY.PEEK[1.1])"), "BINARY.PEEK[1.1]"
    )
    observed = literal(data, "BINARY.PEEK[1.1]")
    if observed != b"message body":
        raise AssertionError(f"decoded nested leaf mismatch: {observed!r}")

    size = require_ok(
        client.fetch(number, "(BINARY.SIZE[1.1])"), "BINARY.SIZE[1.1]"
    )
    if not isinstance(size, list):
        raise AssertionError(f"BINARY.SIZE returned invalid data: {size!r}")
    metadata = b" ".join(value for value in size if isinstance(value, bytes))
    if b"BINARY.SIZE[1.1] 12" not in metadata:
        raise AssertionError(f"decoded size mismatch: {metadata!r}")



def verify_partial_boundaries(
    client: imaplib.IMAP4, number: str
) -> None:
    """Проверяет decode-before-slice и границы partial BINARY на wire."""
    ranges = (
        (0, 4, b"mess"),
        (8, 2, b"bo"),
        (10, 4, b"dy"),
        (12, 4, b""),
        (40, 4, b""),
        (2, 99, b"ssage body"),
    )
    for offset, count, expected in ranges:
        command = f"BINARY.PEEK[1.1]<{offset}.{count}>"
        response = require_ok(
            client.fetch(number, f"({command})"), command
        )
        observed = literal(response, command)
        if observed != expected:
            raise AssertionError(
                f"{command}: expected {expected!r}, got {observed!r}"
            )
        metadata = fetch_metadata(response, command)
        origin = f"BINARY[1.1]<{offset}>".encode("ascii")
        if origin not in metadata:
            raise AssertionError(f"{command}: origin label missing in {metadata!r}")


def options() -> argparse.Namespace:
    """Читает адрес сервера и секрет из аргумента либо окружения."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="localhost")
    parser.add_argument("--imap-port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD"))
    selected = parser.parse_args()
    if selected.password is None:
        raise ValueError("provide --password or LITTERKEEP_BOB_PASSWORD")
    return selected


def main() -> None:
    """APPEND-ит все профили, затем независимо FETCH-ит выбранные части."""
    selected = options()
    fixtures = [
        (profile, *message_fixture(profile)) for profile in MultipartProfile
    ]
    context = ssl.create_default_context(cafile=selected.ca)
    context.check_hostname = False
    with imaplib.IMAP4(selected.host, selected.imap_port, timeout=10) as client:
        client.starttls(ssl_context=context)
        require_ok(client.login("bob", selected.password), "LOGIN")
        for _profile, source, _subject in fixtures:
            require_ok(client.append("INBOX", None, None, source), "APPEND")
        require_ok(client.select("INBOX", readonly=True), "SELECT")
        for profile, _source, subject in fixtures:
            number = find_message(client, subject)
            verify_fetch(client, number, profile)
            verify_partial_boundaries(client, number)
    print("PASS IMAP BINARY nested multipart profiles")


if __name__ == "__main__":
    main()
