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


def pipeline_saved_search_copy(
    client: imaplib.IMAP4, marker: str, destination: str
) -> None:
    """Отправляет SEARCH SAVE и зависимый COPY до чтения любого ответа."""
    search_tag = client._new_tag()
    copy_tag = client._new_tag()
    search = (
        search_tag
        + b' SEARCH RETURN (SAVE) HEADER X-Litterkeep-Fixture "'
        + marker.encode("ascii")
        + b'"\r\n'
    )
    copy = copy_tag + b' COPY $ "' + destination.encode("ascii") + b'"\r\n'
    client.send(search)
    client.send(copy)
    require(client._get_tagged_response(search_tag), "pipelined SEARCH SAVE")
    require(client._get_tagged_response(copy_tag), "pipelined COPY saved result")


def verify_searchres_capability(client: imaplib.IMAP4, phase: str) -> None:
    """Проверяет обязательные capability ESEARCH и SEARCHRES для данного TLS-состояния."""
    capabilities = require(client.capability(), f"CAPABILITY {phase}")
    capability_atoms = b" ".join(capabilities).upper()
    if b"ESEARCH" not in capability_atoms or b"SEARCHRES" not in capability_atoms:
        raise AssertionError(f"SEARCHRES dependencies are not advertised: {capabilities!r}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("host", nargs="?", default="127.0.0.1")
    parser.add_argument("port", nargs="?", type=int, default=2149)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    options = parser.parse_args()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4(options.host, options.port, timeout=10)
    verify_searchres_capability(client, "before STARTTLS")
    client.starttls(ssl_context=context)
    require(client.login("bob", os.environ["LITTERKEEP_BOB_PASSWORD"]), "LOGIN")
    verify_searchres_capability(client, "after STARTTLS")
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
    empty_sequence_copy = f"{mailbox}-empty-sequence-copy"
    require(client.create(empty_sequence_copy), "CREATE empty sequence COPY destination")
    require(client.copy("$", empty_sequence_copy), "COPY empty saved result")
    require(client.select(empty_sequence_copy), "SELECT empty sequence COPY destination")
    empty_sequence = require(client.uid("SEARCH", "ALL"), "SEARCH empty sequence COPY result")
    if response_bytes(empty_sequence).strip():
        raise AssertionError(f"COPY empty saved result copied a message: {empty_sequence!r}")

    sequence_copy = f"{mailbox}-sequence-copy"
    require(client.create(sequence_copy), "CREATE sequence COPY destination")
    require(client.select(mailbox), "SELECT source before sequence COPY")
    saved_search(client, third_marker)
    require(client.copy("$", sequence_copy), "COPY saved sequence result")
    require(client.select(sequence_copy), "SELECT sequence COPY destination")
    copied_sequence = require(client.uid("SEARCH", "ALL"), "SEARCH sequence COPY result")
    if response_bytes(copied_sequence).strip() != b"1":
        raise AssertionError(f"COPY $ did not copy exactly one saved message: {copied_sequence!r}")

    uid_copy = f"{mailbox}-uid-copy"
    require(client.create(uid_copy), "CREATE UID COPY destination")
    require(client.select(mailbox), "SELECT source before UID COPY")
    saved_search(client, first_marker, uid=True)
    require(client.uid("COPY", "$", uid_copy), "UID COPY saved UID result")
    require(client.select(uid_copy), "SELECT UID COPY destination")
    copied_uid = require(client.uid("SEARCH", "ALL"), "SEARCH UID COPY result")
    if response_bytes(copied_uid).strip() != b"1":
        raise AssertionError(f"UID COPY $ did not copy exactly one saved message: {copied_uid!r}")

    pipelined_copy = f"{mailbox}-pipelined-copy"
    require(client.create(pipelined_copy), "CREATE pipelined COPY destination")
    require(client.select(mailbox), "SELECT source before pipelined COPY")
    pipeline_saved_search_copy(client, third_marker, pipelined_copy)
    require(client.select(pipelined_copy), "SELECT pipelined COPY destination")
    pipelined_result = require(
        client.uid("SEARCH", "ALL"), "SEARCH pipelined COPY result"
    )
    if response_bytes(pipelined_result).strip() != b"1":
        raise AssertionError(
            f"pipelined COPY did not observe preceding saved result: {pipelined_result!r}"
        )

    empty_uid_copy = f"{mailbox}-empty-uid-copy"
    require(client.create(empty_uid_copy), "CREATE empty UID COPY destination")
    require(client.select(mailbox), "SELECT source before empty UID COPY")
    saved_search(client, f"{mailbox}-absent", uid=True)
    require(client.uid("COPY", "$", empty_uid_copy), "UID COPY empty saved result")
    require(client.select(empty_uid_copy), "SELECT empty UID COPY destination")
    empty_uid = require(client.uid("SEARCH", "ALL"), "SEARCH empty UID COPY result")
    if response_bytes(empty_uid).strip():
        raise AssertionError(f"UID COPY empty saved result copied a message: {empty_uid!r}")

    require(client.select(mailbox), "SELECT source before expunge")

    require(
        client.uid("STORE", "1,3", "+FLAGS", "(\\Deleted)"),
        "mark two messages deleted",
    )
    saved_search(client, third_marker, uid=True)
    require(client.uid("EXPUNGE", "$"), "UID EXPUNGE saved result")
    remaining = require(client.uid("SEARCH", "ALL"), "UID SEARCH after saved EXPUNGE")
    if response_bytes(remaining).strip() != b"1":
        raise AssertionError(f"UID EXPUNGE $ removed outside saved result: {remaining!r}")
    saved_search(client, f"{mailbox}-absent", uid=True)
    require(client.uid("EXPUNGE", "$"), "UID EXPUNGE empty saved result")
    remaining_after_empty = require(client.uid("SEARCH", "ALL"), "UID SEARCH after empty EXPUNGE")
    if response_bytes(remaining_after_empty).strip() != b"1":
        raise AssertionError(f"empty UID EXPUNGE changed mailbox: {remaining_after_empty!r}")
    client.logout()
    print("PASS imap-searchres: FETCH/STORE/COPY/UID COPY/UID EXPUNGE, mapping, empty sets")


if __name__ == "__main__":
    main()
