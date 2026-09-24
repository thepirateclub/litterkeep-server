"""Проверяет применение сохранённого SEARCH-result в FETCH по STARTTLS."""

import argparse
import imaplib
import os
import ssl
import uuid


def require(result, operation: str) -> object:
    """Останавливает тест на первом tagged-ответе, отличном от OK."""
    status, data = result
    if status != "OK":
        raise AssertionError(f"{operation}: {status} {data!r}")
    return data


def response_bytes(data: object) -> bytes:
    """Собирает tuple литералов imaplib, не теряя FETCH заголовок."""
    match data:
        case bytes():
            return data
        case tuple() | list():
            return b" ".join(response_bytes(part) for part in data)
        case _:
            return b""


def saved_search(client: imaplib.IMAP4, marker: str, uid: bool = False) -> None:
    """Сохраняет результат критерия и не требует SEARCH list в ответе."""
    arguments = ("RETURN", "(SAVE)", "HEADER", "X-Litterkeep-Fixture", f'"{marker}"')
    match uid:
        case True:
            result = client.uid("SEARCH", *arguments)
        case False:
            tag = client._command("SEARCH", *arguments)
            result = client._command_complete("SEARCH", tag)
    require(result, "SEARCH SAVE")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("host", nargs="?", default="127.0.0.1")
    parser.add_argument("port", nargs="?", type=int, default=2149)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    options = parser.parse_args()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4(options.host, options.port, timeout=10)
    client.starttls(ssl_context=context)
    require(client.login("bob", os.environ["LITTERKEEP_BOB_PASSWORD"]), "LOGIN")
    mailbox = f"SearchRes-{uuid.uuid4().hex}"
    require(client.create(mailbox), "CREATE")
    markers = [f"{mailbox}-one", f"{mailbox}-two", f"{mailbox}-three"]
    first_marker, _middle_marker, third_marker = markers
    for marker in markers:
        message = f"X-Litterkeep-Fixture: {marker}\r\nSubject: test\r\n\r\nbody\r\n"
        require(client.append(mailbox, None, None, message.encode("ascii")), "APPEND")
    require(client.select(mailbox), "SELECT")
    require(client.store("2", "+FLAGS", "(\\Deleted)"), "STORE deleted")
    require(client.expunge(), "EXPUNGE")

    saved_search(client, third_marker)
    fetched = require(client.fetch("$", "(UID FLAGS BODY[])"), "FETCH saved sequence result")
    wire = response_bytes(fetched)
    if b"2 (UID 3" not in wire:
        raise AssertionError(f"sequence FETCH did not map saved UID to current sequence: {wire!r}")
    if b"1 (UID" in wire:
        raise AssertionError(f"FETCH returned a message outside saved result: {wire!r}")
    if b"FLAGS (\\Seen" not in wire:
        raise AssertionError(f"BODY on saved result did not persist Seen: {wire!r}")

    saved_search(client, first_marker, uid=True)
    uid_fetched = require(client.uid("FETCH", "$", "(UID FLAGS)"), "UID FETCH saved result")
    uid_wire = response_bytes(uid_fetched)
    if b"1 (UID 1" not in uid_wire:
        raise AssertionError(f"UID FETCH did not use saved UID result: {uid_wire!r}")

    saved_search(client, third_marker)
    require(client.store("$", "+FLAGS", "(\\Flagged)"), "STORE saved sequence result")
    stored = require(client.uid("FETCH", "1:3", "(UID FLAGS)"), "UID FETCH after STORE")
    stored_wire = response_bytes(stored)
    if b"UID 3 FLAGS (\\Flagged" not in stored_wire:
        raise AssertionError(f"STORE $ did not update mapped UID 3: {stored_wire!r}")
    if b"UID 1 FLAGS (\\Flagged" in stored_wire:
        raise AssertionError(f"STORE $ updated a message outside saved result: {stored_wire!r}")

    saved_search(client, first_marker, uid=True)
    require(
        client.uid("STORE", "$", "+FLAGS", "(\\Answered)"),
        "UID STORE saved UID result",
    )
    uid_stored = require(
        client.uid("FETCH", "1:3", "(UID FLAGS)"), "UID FETCH after UID STORE"
    )
    uid_stored_wire = response_bytes(uid_stored)
    if b"UID 1 FLAGS (\\Answered" not in uid_stored_wire:
        raise AssertionError(f"UID STORE $ did not update UID 1: {uid_stored_wire!r}")
    if b"UID 3 FLAGS (\\Answered" in uid_stored_wire:
        raise AssertionError(f"UID STORE $ updated a UID outside saved result: {uid_stored_wire!r}")

    saved_search(client, f"{mailbox}-absent")
    empty_fetch = require(client.fetch("$", "(UID FLAGS)"), "FETCH empty saved result")
    if b" FETCH " in response_bytes(empty_fetch):
        raise AssertionError(f"empty saved result returned a FETCH item: {empty_fetch!r}")
    require(client.store("$", "+FLAGS", "(\\Flagged)"), "STORE empty saved result")
    client.logout()
    print("PASS imap-searchres-fetch: FETCH/STORE sequence and UID mapping, empty result")


if __name__ == "__main__":
    main()
