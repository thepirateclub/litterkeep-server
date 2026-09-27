"""Проверяет UID и sequence number в ответе на UID STORE."""

import argparse
from enum import Enum
import imaplib
import re
import ssl
from pathlib import Path


class FixtureAccount(Enum):
    """Разрешённые имена учётных записей acceptance fixture."""

    BOB = "bob"


class ImapCompletion(Enum):
    """Типизированный tagged completion успешной команды IMAP."""

    OK = "OK"


MESSAGES = (
    b"From: alice@litterkeep.test\r\nSubject: first\r\n\r\nfirst\r\n",
    b"From: alice@litterkeep.test\r\nSubject: second\r\n\r\nsecond\r\n",
)


def read_password(path: Path) -> str:
    """Читает пароль Bob из локальной fixture, не выводя его в терминал."""
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


def require_ok(result: tuple[str, list[object]], label: str) -> list[object]:
    """Возвращает untagged data только после успешного tagged completion."""
    status, data = result
    try:
        completion = ImapCompletion(status)
    except ValueError as error:
        raise AssertionError(f"{label}: неверный IMAP status {status}: {data!r}") from error
    if completion is not ImapCompletion.OK:
        raise AssertionError(f"{label}: ожидался OK, получено {status}: {data!r}")
    return data


def response_bytes(data: list[object]) -> bytes:
    """Собирает байтовые строки ответа imaplib без literal payload."""
    return b" ".join(value for value in data if isinstance(value, bytes))


def verify(host: str, port: int, ca: str, password: str) -> None:
    """Удаляет первое письмо и проверяет UID STORE для второго."""
    context = ssl.create_default_context(cafile=ca)
    client = imaplib.IMAP4(host, port)
    try:
        require_ok(client.starttls(ssl_context=context), "STARTTLS")
        require_ok(client.login("bob", password), "LOGIN")
        for message in MESSAGES:
            require_ok(client.append("INBOX", None, None, message), "APPEND")
        require_ok(client.select("INBOX"), "SELECT")
        require_ok(client.store("1", "+FLAGS", "(\\Deleted)"), "STORE Deleted")
        require_ok(client.uid("EXPUNGE", "1"), "UID EXPUNGE")
        result = require_ok(
            client.uid("STORE", "2", "+FLAGS", "(\\Seen)"), "UID STORE"
        )
        wire = response_bytes(result)
        responses = re.findall(rb"(?:\* [0-9]+ FETCH )?([0-9]+) \(([^)]*)\)", wire)
        if len(responses) != 1:
            raise AssertionError(f"ожидался один FETCH response: {wire!r}")
        sequence, items = next(iter(responses))
        if sequence != b"1":
            raise AssertionError(f"ожидался message sequence number 1: {wire!r}")
        if b"UID 2" not in items:
            raise AssertionError(f"ожидался UID 2: {wire!r}")
        if b"\\Seen" not in items:
            raise AssertionError(
                f"UID STORE должен вернуть флаг \\Seen: {wire!r}"
            )
        uid_items = re.findall(rb"(?:^| )UID ([0-9]+)(?: |$)", items)
        if uid_items != [b"2"]:
            raise AssertionError(f"UID должен присутствовать ровно один раз: {wire!r}")
    finally:
        client.logout()


def main() -> None:
    """Запускает независимую проверку RFC 9051 UID STORE ответа."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    options = parser.parse_args()
    verify(options.host, options.port, options.ca, read_password(options.credentials))
    print("PASS RFC 9051: UID STORE FETCH response carries UID and MSN")


if __name__ == "__main__":
    main()
