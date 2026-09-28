"""Проверяет RFC 9755 UTF8=ACCEPT discovery и рабочие режимы IMAP."""

import argparse
from enum import Enum
import imaplib
from pathlib import Path
import ssl
import uuid


class Completion(Enum):
    """Tagged outcomes, принятые этим независимым клиентом."""

    OK = "OK"
    NO = "NO"
    BYE = "BYE"


class UntaggedResponse(Enum):
    """Разрешённые untagged response kinds этого smoke-test."""

    ENABLED = "ENABLED"


class Capability(Enum):
    """Единственный capability atom, проверяемый этим acceptance."""

    UTF8_ACCEPT = b"UTF8=ACCEPT"


class Account(Enum):
    """Учётная запись из локального acceptance fixture."""

    BOB = "bob"


def arguments() -> argparse.Namespace:
    """Читает endpoint, CA и fixture credentials без передачи пароля в argv."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True, type=Path)
    parser.add_argument("--credentials", required=True, type=Path)
    return parser.parse_args()


def bob_password(path: Path) -> str:
    """Находит только запись Bob в локальном acceptance TSV."""
    for row in path.read_text(encoding="ascii").splitlines():
        account, separator, password = row.partition("\t")
        try:
            parsed_account = Account(account)
        except ValueError:
            continue
        match (parsed_account is Account.BOB, bool(separator), bool(password)):
            case (True, True, True):
                return password
            case _:
                continue
    raise AssertionError("в acceptance fixture отсутствуют credentials Bob")


def require_ok(result: tuple[str, object], operation: str) -> object:
    """Требует tagged OK, не принимая неизвестные статусы."""
    status, data = result
    try:
        observed = Completion(status)
    except ValueError as error:
        raise AssertionError(f"{operation}: {status!r} {data!r}") from error
    if observed is not Completion.OK:
        raise AssertionError(f"{operation}: {status!r} {data!r}")
    return data


def capability(client: imaplib.IMAP4, stage: str) -> None:
    """Проверяет discovery atom во всех релевантных auth/TLS состояниях."""
    response = require_ok(client.capability(), f"CAPABILITY {stage}")
    wire = b" ".join(item for item in response if isinstance(item, bytes))
    if Capability.UTF8_ACCEPT.value not in wire.split():
        raise AssertionError(f"UTF8=ACCEPT отсутствует на этапе {stage}: {wire!r}")


def require_no_utf8_append(client: imaplib.IMAP4, message: bytes) -> None:
    """До ENABLE RFC 9755 требует отказать APPEND с UTF-8 header octets."""
    status, detail = client.append("INBOX", None, None, message)
    try:
        observed = Completion(status)
    except ValueError as error:
        raise AssertionError(f"APPEND до ENABLE: {status!r} {detail!r}") from error
    if observed is not Completion.NO:
        raise AssertionError(f"APPEND до ENABLE должен получить NO: {detail!r}")
    count = require_ok(client.select("INBOX", readonly=True), "SELECT empty INBOX")
    if count != [b"0"]:
        raise AssertionError(f"отклонённое письмо опубликовано: {count!r}")


def verify_utf8_mailbox(client: imaplib.IMAP4) -> None:
    """Проверяет UTF-8 quoted mailbox round-trip и ответный LIST после ENABLE."""
    name = "Utf8-旅行-" + uuid.uuid4().hex
    wire_name = ('"' + name + '"').encode("utf-8")
    tag = client._command("CREATE", wire_name)
    require_ok(client._command_complete("CREATE", tag), "CREATE UTF-8")
    rows = require_ok(client.list(), "LIST UTF-8")
    listing = b"\n".join(item for item in rows if isinstance(item, bytes))
    if name.encode("utf-8") not in listing:
        raise AssertionError(f"LIST не вернул UTF-8 mailbox: {listing!r}")


def verify_header_append_and_fetch(client: imaplib.IMAP4, message: bytes) -> None:
    """Сверяет UTF-8 RFC 5322 header acceptance и ENVELOPE projection."""
    require_ok(client.append("INBOX", None, None, message), "APPEND UTF-8")
    require_ok(client.select("INBOX"), "SELECT INBOX")
    response = require_ok(client.fetch("1", "(ENVELOPE)"), "FETCH ENVELOPE")
    wire = b" ".join(item for item in response if isinstance(item, bytes))
    if "café".encode("utf-8") not in wire:
        raise AssertionError(f"ENVELOPE UTF-8 mode lost Subject: {wire!r}")


def main() -> None:
    """Исполняет capability и UTF-8 behavior через независимый STARTTLS peer."""
    options = arguments()
    client = imaplib.IMAP4(options.host, options.port, timeout=10)
    capability(client, "cleartext")
    client.starttls(ssl_context=ssl.create_default_context(cafile=options.ca))
    capability(client, "TLS")
    require_ok(client.login("bob", bob_password(options.credentials)), "LOGIN")
    capability(client, "authenticated")
    message = b"From: bob@litterkeep.test\r\nSubject: caf\xc3\xa9\r\n\r\nbody"
    require_no_utf8_append(client, message)
    require_ok(client.noop(), "NOOP after rejected APPEND")
    logout_status, _ = client.logout()
    if Completion(logout_status) is not Completion.BYE:
        raise AssertionError(f"неожиданный LOGOUT: {logout_status!r}")

    client = imaplib.IMAP4(options.host, options.port, timeout=10)
    client.starttls(ssl_context=ssl.create_default_context(cafile=options.ca))
    require_ok(client.login("bob", bob_password(options.credentials)), "LOGIN UTF-8")
    capability(client, "authenticated second session")
    enabled = require_ok(client._simple_command("ENABLE", "UTF8=ACCEPT"), "ENABLE UTF8=ACCEPT")
    response_type, enabled_items = client.response("ENABLED")
    try:
        parsed_response = UntaggedResponse(response_type)
    except ValueError as error:
        raise AssertionError(f"ENABLE response kind is invalid: {response_type!r}") from error
    capability_enabled = any(
        Capability.UTF8_ACCEPT.value in item
        for item in enabled_items
        if isinstance(item, bytes)
    )
    if parsed_response is not UntaggedResponse.ENABLED or not capability_enabled:
        raise AssertionError(
            f"ENABLE не подтвердил UTF8=ACCEPT: {enabled!r}, "
            f"{response_type!r} {enabled_items!r}"
        )
    client._encoding = "utf-8"
    capability(client, "enabled")
    verify_utf8_mailbox(client)
    verify_header_append_and_fetch(client, message)
    logout_status, _ = client.logout()
    if Completion(logout_status) is not Completion.BYE:
        raise AssertionError(f"неожиданный LOGOUT: {logout_status!r}")
    print("IMAP RFC 9755 UTF8=ACCEPT capability acceptance passed")


if __name__ == "__main__":
    main()
