"""Проверяет RFC 7888 LITERAL- без серверного continuation."""

import argparse
import imaplib
import os
import ssl
import uuid
from enum import Enum
from pathlib import Path


class ImapCompletion(Enum):
    """Типизированные статусы завершения независимого IMAP-клиента."""

    OK = "OK"


class ImapCapability(Enum):
    """Возможности, которые сервер обязан объявить в этом сценарии."""

    LITERAL_MINUS = b"LITERAL-"


def require(
    status: str, detail: object, expected: ImapCompletion, operation: str
) -> object:
    try:
        observed = ImapCompletion(status.upper())
    except ValueError as error:
        raise AssertionError(f"{operation}: unexpected status {status} {detail!r}") from error
    if observed is not expected:
        raise AssertionError(f"{operation}: {status} {detail!r}")
    return detail


def require_capability(client: imaplib.IMAP4, phase: str) -> None:
    status, response = client.capability()
    require(status, response, ImapCompletion.OK, f"{phase} CAPABILITY")
    capabilities = b" ".join(response).upper().split()
    if ImapCapability.LITERAL_MINUS.value not in capabilities:
        raise AssertionError(f"{phase} capability omitted LITERAL-: {capabilities!r}")


def send_non_sync(
    client: imaplib.IMAP4, command: bytes, response_name: str, payload: bytes
) -> None:
    """Отправляет literal и тело одним write, не ожидая continuation."""
    tag = client._new_tag()
    client.tagged_commands.update({tag: None})
    marker = f"{{{len(payload)}+}}\r\n".encode("ascii")
    client.send(tag + b" " + command + b" " + marker + payload + b"\r\n")
    require(
        *client._command_complete(response_name, tag),
        ImapCompletion.OK,
        f"{response_name} non-sync literal",
    )


def verify_search_literal(client: imaplib.IMAP4) -> None:
    """Проверяет немедленную обработку SEARCH literal в обычном command path."""
    query = b"literal needle"
    send_non_sync(client, b"SEARCH BODY", "SEARCH", query)
    _, result = client.response("SEARCH")
    values = [value for value in result or [] if isinstance(value, bytes)]
    if values != [b"1"]:
        raise AssertionError(f"non-synchronizing SEARCH result mismatch: {values!r}")


def verify_append_literal(client: imaplib.IMAP4, mailbox: str) -> None:
    """Сверяет байты письма после APPEND literal без continuation."""
    payload = b"Subject: LITERAL minus\r\n\r\nliteral needle\r\n"
    send_non_sync(client, f'APPEND "{mailbox}"'.encode(), "APPEND", payload)
    require(*client.select(mailbox), ImapCompletion.OK, "reselect after non-sync APPEND")
    status, rows = client.fetch("2", "(RFC822)")
    require(status, rows, ImapCompletion.OK, "FETCH after non-sync APPEND")
    messages: list[bytes] = []
    for row in rows or []:
        match row:
            case (_, body) if isinstance(body, bytes):
                messages.append(body)
            case _:
                continue
    if messages != [payload]:
        raise AssertionError(f"APPEND payload changed: {messages!r}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=2147)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    parser.add_argument("--credentials", type=Path)
    arguments = parser.parse_args()
    client = imaplib.IMAP4(arguments.host, arguments.port, timeout=10)
    require_capability(client, "pre-TLS")
    client.starttls(ssl_context=ssl.create_default_context(cafile=arguments.ca))
    require_capability(client, "post-TLS")
    password = password_for(arguments.credentials).encode("ascii")
    tag = client._new_tag()
    client.tagged_commands.update({tag: None})
    client.send(tag + b" LOGIN bob {" + str(len(password)).encode() + b"+}\r\n" + password + b"\r\n")
    require(
        *client._command_complete("LOGIN", tag),
        ImapCompletion.OK,
        "LOGIN non-sync literal",
    )
    client.state = "AUTH"

    mailbox = f"LiteralMinus-{uuid.uuid4().hex}"
    require(*client.create(mailbox), ImapCompletion.OK, "CREATE mailbox")
    require(
        *client.append(mailbox, None, None, b"Subject: seed\r\n\r\nliteral needle\r\n"),
        ImapCompletion.OK,
        "seed APPEND",
    )
    require(*client.select(mailbox), ImapCompletion.OK, "SELECT mailbox")
    verify_search_literal(client)
    verify_append_literal(client, mailbox)
    client.logout()
    print("PASS imap-literal-minus: capability, LOGIN, SEARCH and byte-exact APPEND")


def password_for(credentials: Path | None) -> str:
    """Читает случайный тестовый секрет из bootstrap-файла этапа."""
    path = credentials or Path("data/test-accounts.tsv")
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t", maxsplit=1):
            case [account, password] if account == "bob":
                return password
            case _:
                continue
    raise AssertionError(f"missing Bob test account in {path}")


if __name__ == "__main__":
    main()
