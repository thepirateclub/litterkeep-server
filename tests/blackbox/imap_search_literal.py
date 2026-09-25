"""Проверяет IMAP SEARCH sync-literals через независимый STARTTLS клиент."""

import argparse
import imaplib
import os
import ssl
import uuid
from pathlib import Path


def options() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=2146)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    parser.add_argument("--credentials", type=Path)
    return parser.parse_args()


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


def send_literal(client: imaplib.IMAP4, command: bytes, literal: bytes) -> list[bytes]:
    tag = client._new_tag()
    client.tagged_commands[tag] = None
    client.send(tag + b" " + command + b"\r\n")
    continuation = client.readline()
    if not continuation.startswith(b"+"):
        raise AssertionError(f"SEARCH literal continuation missing: {continuation!r}")
    client.send(literal + b"\r\n")
    require(client._command_complete("SEARCH", tag), "OK")
    _, response = client.response("SEARCH")
    return [item for item in (response or []) if item is not None]


def main() -> None:
    selected = options()
    context = ssl.create_default_context(cafile=selected.ca)
    client = imaplib.IMAP4(selected.host, selected.port, timeout=10)
    client.starttls(ssl_context=context)
    password = os.environ.get("LITTERKEEP_BOB_PASSWORD")
    if selected.credentials is not None:
        password = password_for(selected.credentials, "bob")
    if password is None:
        raise AssertionError("bob password is required through credentials or environment")
    require(client.login("bob", password), "OK")
    mailbox = f"SearchLiteral-{uuid.uuid4().hex}"
    require(client.create(mailbox), "OK")
    message = (
        b"Subject: literal subject\r\n"
        b"Content-Type: text/plain; charset=utf-8\r\n"
        b"Content-Transfer-Encoding: 8bit\r\n\r\n"
        b"literal needle " + "Привет\r\n".encode("utf-8")
    )
    require(client.append(mailbox, None, None, message), "OK")
    require(client.select(mailbox), "OK")

    baseline = require(client.search("UTF-8", "BODY", '"literal needle"'), "OK")
    if baseline != [b"1"]:
        raise AssertionError(f"UTF-8 fixture baseline failed: {baseline!r}")
    body = send_literal(client, b"SEARCH BODY {14}", b"literal needle")
    if body != [b"1"]:
        raise AssertionError(f"BODY literal mismatch: {body!r}")

    uid_body = send_literal(client, b"UID SEARCH BODY {14}", b"literal needle")
    if uid_body != [b"1"]:
        raise AssertionError(f"UID SEARCH literal mismatch: {uid_body!r}")

    unicode_literal = "Привет".encode("utf-8")
    unicode_result = send_literal(
        client,
        f"SEARCH BODY {{{len(unicode_literal)}}}".encode("ascii"),
        unicode_literal,
    )
    if unicode_result != [b"1"]:
        raise AssertionError(f"UTF-8 BODY literal mismatch: {unicode_result!r}")

    empty = send_literal(client, b"SEARCH BODY {0}", b"")
    if empty != [b"1"]:
        raise AssertionError(f"empty BODY literal mismatch: {empty!r}")
    client.logout()
    print("PASS imap-search-literal: BODY, UID, UTF-8, empty")


def password_for(path: Path, username: str) -> str:
    """Читает пароль из локального bootstrap-файла без передачи через argv."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t", maxsplit=1):
            case [account, password] if account == username:
                return password
            case _:
                continue
    raise AssertionError(f"missing test account: {username}")


if __name__ == "__main__":
    main()
