"""Проверяет обязательный UID item в ответе на UID FETCH."""

import argparse
from enum import Enum
import imaplib
import re
import ssl
from pathlib import Path


class FixtureAccount(Enum):
    """Типизированный аккаунт локального acceptance fixture."""

    BOB = "bob"


MESSAGE = (
    b"From: alice@litterkeep.test\r\n"
    b"To: bob@litterkeep.test\r\n"
    b"Subject: UID FETCH implicit item\r\n"
    b"\r\n"
    b"RFC 9051 response data item check\r\n"
)


def read_bob_password(credentials: Path) -> str:
    """Читает секрет из fixture, не печатая его и не передавая через argv."""
    for record in credentials.read_text(encoding="ascii").splitlines():
        name, separator, password = record.partition("\t")
        try:
            account = FixtureAccount(name)
        except ValueError:
            continue
        if account is FixtureAccount.BOB and separator:
            return password
    raise ValueError(f"Bob отсутствует в fixture {credentials}")


def response_bytes(data: list[object]) -> bytes:
    """Собирает только FETCH framing; RFC822 literal здесь не запрашивается."""
    parts = (value[0] if isinstance(value, tuple) else value for value in data)
    return b" ".join(part for part in parts if isinstance(part, bytes))


def require_ok(result: tuple[str, list[object]], label: str) -> list[object]:
    """Проверяет tagged completion без потери server response data."""
    status, data = result
    if status != "OK":
        raise AssertionError(f"{label}: ожидался OK, получено {status}: {data!r}")
    return data


def verify(host: str, port: int, ca: str, password: str) -> None:
    """Сравнивает обычный FETCH и UID FETCH для одного сохранённого письма."""
    context = ssl.create_default_context(cafile=ca)
    client = imaplib.IMAP4(host, port)
    try:
        require_ok(client.starttls(ssl_context=context), "STARTTLS")
        require_ok(client.login("bob", password), "LOGIN")
        require_ok(client.append("INBOX", None, None, MESSAGE), "APPEND")
        require_ok(client.select("INBOX"), "SELECT")
        _, search_data = client.uid("SEARCH", None, "ALL")
        identifiers = b" ".join(
            value for value in search_data if isinstance(value, bytes)
        ).split()
        if len(identifiers) != 1:
            raise AssertionError(f"ожидалось ровно одно письмо, UID set={identifiers!r}")
        uid = next(iter(identifiers), None)
        if uid is None:
            raise AssertionError("UID SEARCH вернул пустой результат")
        ordinary = require_ok(client.fetch("1", "(FLAGS RFC822.SIZE)"), "FETCH")
        ordinary_wire = response_bytes(ordinary)
        if re.search(rb"(?:^|[ (])UID [0-9]+(?=[ )])", ordinary_wire):
            raise AssertionError(f"обычный FETCH неожиданно включил UID: {ordinary_wire!r}")
        uid_result = require_ok(
            client.uid("FETCH", uid.decode("ascii"), "(FLAGS RFC822.SIZE)"),
            "UID FETCH without requested UID item",
        )
        uid_wire = response_bytes(uid_result)
        uid_items = re.findall(rb"(?:^|[ (])UID ([0-9]+)(?=[ )])", uid_wire)
        if uid_items != [uid]:
            raise AssertionError(
                f"UID FETCH должен вернуть UID ровно один раз: {uid_wire!r}"
            )
    finally:
        client.logout()


def main() -> None:
    """Запускает независимый RFC 9051 STARTTLS acceptance."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    options = parser.parse_args()
    password = read_bob_password(options.credentials)
    verify(options.host, options.port, options.ca, password)
    print("PASS RFC 9051: UID FETCH implicitly returns exactly one UID item")


if __name__ == "__main__":
    main()
