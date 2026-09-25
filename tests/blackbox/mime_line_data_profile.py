"""Проверяет RFC 2045 7bit/8bit line-data правила через IMAP BINARY."""

import argparse
import dataclasses
import enum
import imaplib
import os
import socket
import ssl
import uuid


class ImapStatus(enum.Enum):
    """Ожидаемые tagged status для BINARY FETCH."""

    OK = "OK"
    NO = "NO"


class TransferLabel(enum.Enum):
    """Разрешённые профили identity transfer encoding в fixture."""

    SEVEN_BIT = "7bit"
    EIGHT_BIT = "8bit"


class FetchOutcome(enum.Enum):
    """Ожидаемый результат MIME-проверки при BINARY декодировании."""

    ACCEPT = enum.auto()
    REJECT = enum.auto()


class MimeCase(enum.Enum):
    """Идентификаторы независимых RFC 2045 fixtures."""

    SEVEN_BIT_LIMIT = enum.auto()
    SEVEN_BIT_OVER_LIMIT = enum.auto()
    EIGHT_BIT_HIGH_OCTET = enum.auto()
    SEVEN_BIT_HIGH_OCTET = enum.auto()
    SEVEN_BIT_NUL = enum.auto()
    EIGHT_BIT_NUL = enum.auto()
    EIGHT_BIT_BARE_LF = enum.auto()
    SEVEN_BIT_BARE_CR = enum.auto()


@dataclasses.dataclass(frozen=True)
class MimeFixture:
    """Одно письмо для проверки line-data и его ожидаемый результат."""

    case: MimeCase
    transfer: TransferLabel
    body: bytes
    outcome: FetchOutcome


def require_ok(response: tuple[str, list[object]], operation: str) -> list[object]:
    """Проверяет успешный tagged status через enum."""
    status, data = response
    if ImapStatus(status) is not ImapStatus.OK:
        raise AssertionError(f"{operation} failed: {status!r} {data!r}")
    return data


def options() -> argparse.Namespace:
    """Считывает TLS endpoint и bootstrap credentials для Bob."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--imap-port", type=int, default=2146)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    parser.add_argument("--bob-password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD"))
    parsed = parser.parse_args()
    if parsed.bob_password is None:
        raise ValueError("bootstrap Bob password must be supplied through environment")
    return parsed


def fixtures() -> list[MimeFixture]:
    """Возвращает граничный valid body и запрещённые 7bit/8bit octets."""
    return [
        MimeFixture(
            MimeCase.SEVEN_BIT_LIMIT,
            TransferLabel.SEVEN_BIT,
            b"A" * 998 + b"\r\n",
            FetchOutcome.ACCEPT,
        ),
        MimeFixture(
            MimeCase.SEVEN_BIT_OVER_LIMIT,
            TransferLabel.SEVEN_BIT,
            b"A" * 999 + b"\r\n",
            FetchOutcome.REJECT,
        ),
        MimeFixture(
            MimeCase.EIGHT_BIT_HIGH_OCTET,
            TransferLabel.EIGHT_BIT,
            b"octet-\x80\xff\r\n",
            FetchOutcome.ACCEPT,
        ),
        MimeFixture(
            MimeCase.SEVEN_BIT_HIGH_OCTET,
            TransferLabel.SEVEN_BIT,
            b"octet-\x80\r\n",
            FetchOutcome.REJECT,
        ),
        MimeFixture(
            MimeCase.SEVEN_BIT_NUL,
            TransferLabel.SEVEN_BIT,
            b"nul-\x00\r\n",
            FetchOutcome.REJECT,
        ),
        MimeFixture(
            MimeCase.EIGHT_BIT_NUL,
            TransferLabel.EIGHT_BIT,
            b"nul-\x00\r\n",
            FetchOutcome.REJECT,
        ),
        MimeFixture(
            MimeCase.EIGHT_BIT_BARE_LF,
            TransferLabel.EIGHT_BIT,
            b"bare-lf\n",
            FetchOutcome.REJECT,
        ),
        MimeFixture(
            MimeCase.SEVEN_BIT_BARE_CR,
            TransferLabel.SEVEN_BIT,
            b"bare-cr\rX\r\n",
            FetchOutcome.REJECT,
        ),
    ]


def requires_exact_literal(fixture: MimeFixture) -> bool:
    """Выбирает octets, которые imaplib APPEND канонизирует до отправки."""
    match fixture.case:
        case MimeCase.EIGHT_BIT_BARE_LF | MimeCase.SEVEN_BIT_BARE_CR:
            return True
        case _:
            return False


def message(fixture: MimeFixture, identity: str) -> bytes:
    """Формирует MIME entity, не меняя проверяемые transfer-data octets."""
    return (
        b"From: alice@litterkeep.test\r\n"
        b"To: bob@litterkeep.test\r\n"
        b"Subject: "
        + fixture.case.name.lower().encode("ascii")
        + b"-"
        + identity.encode("ascii")
        + b"\r\nContent-Type: application/octet-stream\r\n"
        + b"Content-Transfer-Encoding: "
        + fixture.transfer.value.encode("ascii")
        + b"\r\n\r\n"
        + fixture.body
    )


def append_fixtures(
    client: imaplib.IMAP4,
    mailbox: str,
    values: list[MimeFixture],
    identity: str,
) -> None:
    """Сохраняет wire fixtures через IMAP APPEND, обходя SMTP line cap."""
    require_ok(client.create(mailbox), "CREATE fixture mailbox")
    for fixture in values:
        require_ok(
            client.append(mailbox, None, None, message(fixture, identity)),
            f"APPEND {fixture.case.name}",
        )


def read_tagged(stream, tag: bytes) -> bytes:
    """Читает ответ до matching tagged line, не теряя CRLF и status text."""
    response = bytearray()
    while True:
        line = stream.readline()
        if not line:
            raise AssertionError("server closed before tagged IMAP response")
        response.extend(line)
        if line.startswith(tag + b" "):
            return bytes(response)


def append_exact_octets(
    settings: argparse.Namespace,
    mailbox: str,
    raw: bytes,
) -> None:
    """Передаёт один literal напрямую, не нормализуя bare LF клиентской библиотекой."""
    tag = b"Q001"
    context = ssl.create_default_context(cafile=settings.ca)
    connection = socket.create_connection(("127.0.0.1", settings.imap_port), timeout=10)
    stream = connection.makefile("rb")
    try:
        greeting = stream.readline()
        if not greeting.startswith(b"* OK"):
            raise AssertionError(f"unexpected IMAP greeting: {greeting!r}")
        connection.sendall(tag + b" STARTTLS\r\n")
        starttls_reply = read_tagged(stream, tag)
        if not starttls_reply.startswith(tag + b" OK "):
            raise AssertionError(f"STARTTLS failed: {starttls_reply!r}")
        stream.close()
        connection = context.wrap_socket(connection, server_hostname="127.0.0.1")
        stream = connection.makefile("rb")
        login_tag = b"Q002"
        password = settings.bob_password.encode("ascii")
        connection.sendall(login_tag + b" LOGIN bob " + password + b"\r\n")
        login_reply = read_tagged(stream, login_tag)
        if b" OK " not in login_reply:
            raise AssertionError(f"LOGIN failed: {login_reply!r}")
        append_tag = b"Q003"
        length = str(len(raw)).encode("ascii")
        command = append_tag + b" APPEND " + mailbox.encode("ascii") + b" {" + length + b"}\r\n"
        connection.sendall(command)
        continuation = stream.readline()
        if not continuation.startswith(b"+"):
            raise AssertionError(f"APPEND literal was not requested: {continuation!r}")
        connection.sendall(raw + b"\r\n")
        append_reply = read_tagged(stream, append_tag)
        if b" OK " not in append_reply:
            raise AssertionError(f"APPEND failed: {append_reply!r}")
    finally:
        stream.close()
        connection.close()


def literal_payload(records: list[object]) -> bytes:
    """Извлекает literal из FETCH tuple без зависимости от record-позиции."""
    values = [
        value
        for record in records
        if isinstance(record, tuple)
        for value in reversed(record)
        if isinstance(value, bytes)
    ]
    return next(iter(values), b"")


def verify_fetch(
    client: imaplib.IMAP4,
    fixtures_to_check: list[MimeFixture],
    identifiers: list[bytes],
) -> None:
    """Сравнивает decoded bytes у валидных случаев и отказ у malformed body."""
    if len(identifiers) != len(fixtures_to_check):
        raise AssertionError(
            f"expected {len(fixtures_to_check)} fixtures, found {len(identifiers)} UIDs"
        )
    for fixture, identifier in zip(fixtures_to_check, identifiers, strict=True):
        status, records = client.uid("FETCH", identifier, "(BINARY.PEEK[1])")
        actual_status = ImapStatus(status)
        match fixture.outcome:
            case FetchOutcome.ACCEPT:
                if actual_status is not ImapStatus.OK:
                    raise AssertionError(
                        f"{fixture.case.name}: valid data rejected: {records!r}"
                    )
                if literal_payload(records) != fixture.body:
                    raise AssertionError(f"{fixture.case.name}: decoded octets differ")
            case FetchOutcome.REJECT:
                if actual_status is not ImapStatus.NO:
                    source = client.uid("FETCH", identifier, "(RFC822)")
                    raise AssertionError(
                        f"{fixture.case.name}: malformed data was not rejected: "
                        f"{records!r}; raw={source!r}"
                    )


def main() -> None:
    """Выполняет APPEND и BINARY acceptance с независимым TLS-клиентом."""
    settings = options()
    identity = uuid.uuid4().hex
    mailbox = "MimeLines-" + identity
    values = fixtures()
    exact_fixtures = [fixture for fixture in values if requires_exact_literal(fixture)]
    library_fixtures = [fixture for fixture in values if not requires_exact_literal(fixture)]
    ordered_fixtures = [*library_fixtures, *exact_fixtures]
    context = ssl.create_default_context(cafile=settings.ca)
    with imaplib.IMAP4("127.0.0.1", settings.imap_port, timeout=10) as client:
        client.starttls(ssl_context=context)
        require_ok(client.login("bob", settings.bob_password), "LOGIN")
        append_fixtures(client, mailbox, library_fixtures, identity)
        for fixture in exact_fixtures:
            append_exact_octets(settings, mailbox, message(fixture, identity))
        require_ok(client.select(mailbox, readonly=True), "SELECT fixture mailbox")
        search = require_ok(client.uid("SEARCH", None, "ALL"), "UID SEARCH")
        identifiers = b" ".join(item for item in search if isinstance(item, bytes)).split()
        verify_fetch(client, ordered_fixtures, identifiers)
    print("PASS mime-line-data-profile: IMAP APPEND and BINARY.PEEK")


if __name__ == "__main__":
    main()
