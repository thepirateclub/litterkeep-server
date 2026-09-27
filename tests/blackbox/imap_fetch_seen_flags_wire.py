"""Проверяет FLAGS в ответе BODY FETCH, который меняет Seen."""

import argparse
from enum import Enum
import imaplib
import re
import ssl
from pathlib import Path
import uuid


class FixtureAccount(Enum):
    """Разрешённый владелец независимой acceptance-сессии."""

    BOB = "bob"


class ImapCompletion(Enum):
    """Успешный tagged completion IMAP-команды."""

    OK = "OK"


MESSAGE = b"From: alice@litterkeep.test\r\nSubject: Seen response\r\n\r\nbody\r\n"
FLAGS_ITEM = re.compile(rb"(?:^| )FLAGS \(([^)]*)\)")


def read_password(path: Path) -> str:
    """Читает fixture secret, не выводя его в командную строку."""
    for record in path.read_text(encoding="ascii").splitlines():
        name, separator, password = record.partition("\t")
        try:
            account = FixtureAccount(name)
        except ValueError:
            continue
        if account is not FixtureAccount.BOB:
            continue
        if not separator:
            continue
        return password
    raise ValueError(f"Bob отсутствует в fixture {path}")


def require_ok(response: tuple[str, list[object]], label: str) -> list[object]:
    """Проверяет tagged completion без потери untagged FETCH данных."""
    status, data = response
    try:
        completion = ImapCompletion(status)
    except ValueError as error:
        raise AssertionError(f"{label}: {status!r} {data!r}") from error
    if completion is not ImapCompletion.OK:
        raise AssertionError(f"{label}: {status!r} {data!r}")
    return data


def response_header(data: list[object]) -> bytes:
    """Собирает FETCH framing, исключая literal payload из сравнения."""
    parts: list[bytes] = []
    for value in data:
        match value:
            case (header, _literal) if isinstance(header, bytes):
                parts.append(header)
            case bytes():
                parts.append(value)
            case _:
                continue
    return b" ".join(parts)


def flags_items(header: bytes) -> list[bytes]:
    """Извлекает FLAGS items из FETCH framing."""
    return [match.group(1) for match in FLAGS_ITEM.finditer(header)]


def verify(host: str, port: int, ca: str, password: str) -> None:
    """Сравнивает BODY.PEEK, Seen-changing BODY и повторный BODY."""
    context = ssl.create_default_context(cafile=ca)
    mailbox = f"SeenFlags{uuid.uuid4().hex}"
    client = imaplib.IMAP4(host, port)
    try:
        require_ok(client.starttls(ssl_context=context), "STARTTLS")
        require_ok(client.login("bob", password), "LOGIN")
        require_ok(client.create(mailbox), "CREATE")
        require_ok(client.append(mailbox, None, None, MESSAGE), "APPEND")
        require_ok(client.select(mailbox), "SELECT")
        peek = require_ok(client.fetch("1", "(BODY.PEEK[])"), "BODY.PEEK")
        if flags_items(response_header(peek)):
            raise AssertionError("BODY.PEEK не должен менять или добавлять FLAGS")
        first_read = require_ok(client.fetch("1", "(BODY[])"), "BODY")
        first_flags = flags_items(response_header(first_read))
        if len(first_flags) != 1:
            raise AssertionError(f"BODY должен вернуть FLAGS один раз: {first_read!r}")
        if b"\\Seen" not in first_flags[0]:
            raise AssertionError(f"BODY FLAGS не отражает Seen: {first_flags!r}")
        repeated_read = require_ok(client.fetch("1", "(BODY[])"), "repeated BODY")
        if flags_items(response_header(repeated_read)):
            raise AssertionError("повторный BODY не менял FLAGS, но добавил их")
    finally:
        client.logout()


def main() -> None:
    """Запускает независимое STARTTLS acceptance RFC 9051 §6.4.5."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    options = parser.parse_args()
    verify(options.host, options.port, options.ca, read_password(options.credentials))
    print("PASS RFC 9051: BODY FETCH reports FLAGS changed to Seen")


if __name__ == "__main__":
    main()
