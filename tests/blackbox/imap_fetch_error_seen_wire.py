"""Проверяет отсутствие Seen mutation при ошибочном составном FETCH."""

import argparse
import csv
from enum import Enum
import imaplib
import re
import ssl
import uuid


class ImapStatus(Enum):
    """Tagged completion states, допустимые отдельными проверками."""

    OK = "OK"
    NO = "NO"


def require_ok(response: tuple[str, object], operation: str) -> object:
    """Отклоняет любой неожиданный tagged статус штатным AssertionError."""
    status, detail = response
    try:
        observed = ImapStatus(status)
    except ValueError as error:
        raise AssertionError(f"{operation}: {status!r} {detail!r}") from error
    if observed is not ImapStatus.OK:
        raise AssertionError(f"{operation}: {status!r} {detail!r}")
    return detail


def password(path: str) -> str:
    """Читает Bob secret из TSV, не выводя его в командную строку или отчёт."""
    with open(path, encoding="utf-8-sig", newline="") as source:
        credentials = csv.reader(source, delimiter="\t")
        for account, secret, *_ in credentials:
            if account == "bob":
                return secret
    raise ValueError("Bob acceptance credential is absent")


def require_unknown_cte(client: imaplib.IMAP4, number: str) -> None:
    """Требует tagged NO с RFC response code UNKNOWN-CTE."""
    try:
        status, detail = client.fetch(number, "(BODY[] BINARY[1])")
    except imaplib.IMAP4.error as error:
        tagged = re.search(r"FETCH command error: ([A-Z]+)", str(error))
        if tagged is None:
            raise AssertionError(f"FETCH failed without tagged status: {error!r}") from error
        status, detail = tagged.group(1), str(error)
    try:
        observed = ImapStatus(status)
    except ValueError as error:
        raise AssertionError(f"unexpected FETCH status: {status!r} {detail!r}") from error
    if observed is not ImapStatus.NO:
        raise AssertionError(f"expected tagged NO, got {status!r} {detail!r}")
    if re.search(r"\[UNKNOWN-CTE\]", str(detail)) is None:
        raise AssertionError(f"FETCH NO did not identify UNKNOWN-CTE: {detail!r}")


def message(subject: str) -> bytes:
    """Формирует небольшой RFC 5322 текст с неизвестным MIME CTE token."""
    return (
        f"Subject: {subject}\r\n"
        "Content-Type: text/plain; charset=utf-8\r\n"
        "Content-Transfer-Encoding: x-litterkeep-unknown\r\n"
        "\r\n"
        "unread body\r\n"
    ).encode("ascii")


def run(settings: argparse.Namespace) -> None:
    """APPEND-ит unread fixture и проверяет FETCH failure без Seen перехода."""
    subject = f"fetch seen atomicity {uuid.uuid4().hex}"
    context = ssl.create_default_context(cafile=settings.ca)
    context.check_hostname = False
    with imaplib.IMAP4(settings.host, settings.port, timeout=10) as client:
        client.starttls(ssl_context=context)
        require_ok(client.login("bob", password(settings.credentials)), "LOGIN")
        require_ok(client.append("INBOX", None, None, message(subject)), "APPEND")
        require_ok(client.select("INBOX"), "SELECT writable INBOX")
        found = require_ok(
            client.search(None, "HEADER", "SUBJECT", f'"{subject}"'), "SEARCH"
        )
        if not isinstance(found, list):
            raise AssertionError(f"invalid SEARCH response: {found!r}")
        numbers = b" ".join(item for item in found if isinstance(item, bytes)).split()
        match numbers:
            case [number_bytes]:
                number = number_bytes.decode("ascii")
            case _:
                raise AssertionError(f"expected one message, found {numbers!r}")
        require_unknown_cte(client, number)
        require_ok(client.noop(), "NOOP after failed FETCH")
        flags = require_ok(client.fetch(number, "(FLAGS)"), "FETCH FLAGS")
        if not isinstance(flags, list):
            raise AssertionError(f"invalid FLAGS response: {flags!r}")
        metadata = b" ".join(item for item in flags if isinstance(item, bytes))
        if b"\\Seen" in metadata:
            raise AssertionError(f"failed FETCH mutated Seen: {metadata!r}")


def main() -> None:
    """Разбирает локальный TLS endpoint и запускает независимый wire acceptance."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", required=True)
    run(parser.parse_args())
    print("PASS IMAP failed FETCH leaves message unseen")


if __name__ == "__main__":
    main()
