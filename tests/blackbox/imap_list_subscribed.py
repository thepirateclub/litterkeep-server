"""Проверяет RFC LIST (SUBSCRIBED) через независимый TLS IMAP-клиент."""

import argparse
import imaplib
import os
import re
import ssl
import uuid
from itertools import pairwise


def require(status: str, detail: object, expected: str, operation: str) -> object:
    if status != expected:
        raise AssertionError(f"{operation}: {status} {detail!r}")
    return detail


def require_list_capabilities(client: imaplib.IMAP4, phase: str) -> None:
    """Проверяет объявление LIST extensions до отправки их клиентом."""
    status, responses = client.capability()
    require(status, responses, "OK", f"{phase} CAPABILITY")
    tokens = b" ".join(responses).upper().split()
    for capability in (b"LIST-EXTENDED", b"LIST-STATUS"):
        if capability not in tokens:
            raise AssertionError(
                f"{phase} CAPABILITY omitted {capability!r}: {tokens!r}"
            )


def list_command(client: imaplib.IMAP4, command: bytes, expected: str = "OK") -> bytes:
    tagged_command(client, command, expected)
    values = client.untagged_responses.pop("LIST", [])
    return b"\n".join(value for value in values if isinstance(value, bytes))


def tagged_command(client: imaplib.IMAP4, command: bytes, expected: str) -> None:
    tag = client._new_tag()
    client.send(tag + b" " + command + b"\r\n")
    status, detail = client._get_tagged_response(tag)
    require(status, detail, expected, command.decode("utf-8"))


def ordered_list_command(client: imaplib.IMAP4, command: bytes) -> list[bytes]:
    """Сохраняет wire-порядок untagged ответов, чтобы проверить LIST/STATUS соседство."""
    responses: list[bytes] = []
    original = client._get_response

    def capture(*args: object, **kwargs: object) -> bytes | None:
        response = original(*args, **kwargs)
        if response is not None:
            responses.append(response)
        return response

    client._get_response = capture
    try:
        tagged_command(client, command, "OK")
    finally:
        client._get_response = original
    return responses


def require_adjacent(
    responses: list[bytes], list_prefix: bytes, status_prefix: bytes, operation: str
) -> None:
    matched = any(
        first.startswith(list_prefix) and second.startswith(status_prefix)
        for first, second in pairwise(responses)
    )
    if not matched:
        raise AssertionError(f"{operation}: LIST and STATUS were not adjacent: {responses!r}")


def require_record(records: bytes, expected: bytes, operation: str) -> None:
    if expected not in records:
        raise AssertionError(f"{operation}: expected {expected!r} in {records!r}")


def reject_record(records: bytes, unexpected: bytes, operation: str) -> None:
    if unexpected in records:
        raise AssertionError(f"{operation}: unexpected {unexpected!r} in {records!r}")


def verify_utf8_status(client: imaplib.IMAP4, suffix: str) -> None:
    """Сверяет RFC 9755 UTF-8 mailbox в RFC 5258 LIST RETURN STATUS ответах."""
    tagged_command(client, b"ENABLE UTF8=ACCEPT", "OK")
    name = f"Status Café {suffix}"
    tagged_command(client, f'CREATE "{name}"'.encode("utf-8"), "OK")
    command = (
        f'LIST () "" "{name}" RETURN '
        "(STATUS (MESSAGES UIDNEXT UIDVALIDITY SIZE))"
    ).encode("utf-8")
    responses = ordered_list_command(client, command)
    expected_list = f'* LIST () "/" "{name}"'.encode("utf-8")
    expected_status = re.compile(
        rb'^\* STATUS "' + re.escape(name.encode("utf-8"))
        + rb'" \(MESSAGES 0 UIDNEXT 1 UIDVALIDITY [0-9]+ SIZE 0\)$'
    )
    matched = any(
        first == expected_list and expected_status.fullmatch(second) is not None
        for first, second in pairwise(responses)
    )
    if not matched:
        raise AssertionError(f"UTF-8 LIST RETURN STATUS mismatch: {responses!r}")


def verify_selected_recent(client: imaplib.IMAP4, name: str) -> None:
    """Проверяет RECENT из session-owned SELECT после передачи durable claim."""
    require(*client.select(name), "OK", "SELECT recent fixture")
    status, rows = client.status(name, "(RECENT)")
    require(status, rows, "OK", "STATUS RECENT in selected mailbox")
    if not any(b"RECENT 1" in row for row in rows or []):
        raise AssertionError(f"STATUS lost session-owned RECENT: {rows!r}")
    command = (
        f'LIST () "" "{name}" RETURN (STATUS (MESSAGES RECENT))'
    ).encode("ascii")
    responses = ordered_list_command(client, command)
    require_adjacent(
        responses,
        f'* LIST () "/" "{name}"'.encode("ascii"),
        f'* STATUS "{name}" (MESSAGES 1 RECENT 1)'.encode("ascii"),
        "STATUS reports selected session RECENT",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("host", nargs="?", default="127.0.0.1")
    parser.add_argument("port", nargs="?", type=int, default=2149)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    options = parser.parse_args()
    trust = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4(options.host, options.port, timeout=10)
    require_list_capabilities(client, "pre-TLS")
    client.starttls(ssl_context=trust)
    require_list_capabilities(client, "post-TLS")
    password = os.environ["LITTERKEEP_BOB_PASSWORD"]
    require(*client.login("bob", password), "OK", "LOGIN")
    suffix = uuid.uuid4().hex
    present_name = f"SubscribedPresent{suffix}"
    missing_name = f"SubscribedMissing{suffix}"
    child_name = f"{present_name}/Nested"
    grandchild_name = f"{child_name}/January"
    phantom_child = f"Phantom{suffix}/Nested"
    require(*client.create(present_name), "OK", "CREATE")
    status, append_status = client.status(present_name, "(APPENDLIMIT)")
    require(status, append_status, "OK", "STATUS APPENDLIMIT")
    if not any(b"APPENDLIMIT 26214400" in row for row in append_status or []):
        raise AssertionError(f"STATUS omitted APPENDLIMIT: {append_status!r}")
    require(*client.create(child_name), "OK", "CREATE nested")
    require(*client.create(grandchild_name), "OK", "CREATE grandchild")
    status_message = b"Subject: status fixture\r\n\r\nstatus body\r\n"
    require(
        *client.append(present_name, "(\\Deleted)", None, status_message),
        "OK",
        "APPEND status fixture",
    )
    require(*client.subscribe(present_name), "OK", "SUBSCRIBE existing")
    require(*client.subscribe(missing_name), "OK", "SUBSCRIBE missing")
    require(*client.subscribe(child_name), "OK", "SUBSCRIBE nested")
    require(*client.subscribe(phantom_child), "OK", "SUBSCRIBE phantom child")

    root_status, reference_root = client.list(present_name + "/", "")
    require(root_status, reference_root, "OK", "reference root discovery")
    if reference_root != [f'(\\Noselect) "/" "{present_name}"'.encode()]:
        raise AssertionError(f"reference root: {reference_root!r}")

    relative = list_command(
        client, f'LIST "{present_name}/" "%"'.encode()
    )
    require_record(relative, f'"{child_name}"'.encode(), "reference one-level child")
    reject_record(relative, grandchild_name.encode(), "reference percent stays one level")
    relative_recursive = list_command(
        client, f'LIST "{present_name}" "*"'.encode()
    )
    require_record(relative_recursive, f'"{grandchild_name}"'.encode(), "reference star crosses levels")
    relative_return = list_command(
        client,
        f'LIST "{present_name}/" "%" RETURN (CHILDREN)'.encode(),
    )
    require_record(
        relative_return,
        f'(\\HasChildren) "/" "{child_name}"'.encode(),
        "reference with return options",
    )
    special_parent = f"Special{suffix}"
    special_child = f"{special_parent}/Sent"
    require(*client.create(special_parent), "OK", "CREATE special parent")
    tagged_command(
        client,
        f'CREATE "{special_child}" (USE (\\Sent))'.encode(),
        "OK",
    )

    records = list_command(client, b'LIST (SUBSCRIBED) "" "*"')
    present = f'(\\Subscribed) "/" "{present_name}"'.encode()
    missing = f'(\\NonExistent \\Subscribed) "/" "{missing_name}"'.encode()
    require_record(records, present, "existing subscription")
    require_record(records, missing, "missing subscription")
    parent = (
        f'(\\Subscribed) "/" "{present_name}" '
        '("CHILDINFO" ("SUBSCRIBED"))'
    ).encode()
    phantom = (
        f'(\\NonExistent) "/" "Phantom{suffix}" '
        '("CHILDINFO" ("SUBSCRIBED"))'
    ).encode()
    recursive = list_command(
        client, b'LIST (SUBSCRIBED RECURSIVEMATCH) "" "%"'
    )
    require_record(recursive, parent, "recursive existing parent")
    require_record(recursive, phantom, "recursive nonexistent parent")
    all_levels = list_command(
        client, b'LIST (RECURSIVEMATCH SUBSCRIBED) "" "*"'
    )
    reject_record(all_levels, b'"CHILDINFO"', "redundant CHILDINFO")
    require_record(all_levels, f'"{child_name}"'.encode(), "recursive child")
    special = list_command(
        client, b'LIST (SPECIAL-USE RECURSIVEMATCH) "" "%"'
    )
    special_parent_record = (
        f'() "/" "{special_parent}" '
        '("CHILDINFO" ("SPECIAL-USE"))'
    ).encode()
    require_record(special, special_parent_record, "recursive special-use parent")
    children = list_command(client, b'LIST "" "*" RETURN (CHILDREN)')
    require_record(
        children,
        f'(\\HasChildren) "/" "{present_name}"'.encode(),
        "parent with child",
    )
    require_record(
        children,
        f'(\\HasChildren) "/" "{child_name}"'.encode(),
        "nested mailbox with child",
    )
    require_record(
        children,
        f'(\\HasNoChildren) "/" "{grandchild_name}"'.encode(),
        "leaf mailbox",
    )
    reject_record(children, missing_name.encode(), "ordinary list excludes missing subscription")
    subscribed_children = list_command(
        client, b'LIST "" "*" RETURN (SUBSCRIBED CHILDREN)'
    )
    require_record(
        subscribed_children,
        f'(\\Subscribed \\HasChildren) "/" "{present_name}"'.encode(),
        "combined return options",
    )
    require_record(
        subscribed_children,
        f'(\\HasChildren) "/" "{special_parent}"'.encode(),
        "SUBSCRIBED return does not select a non-subscribed mailbox",
    )
    reject_record(
        subscribed_children,
        missing_name.encode(),
        "SUBSCRIBED return does not select nonexistent mailbox",
    )
    status_command = (
        b'LIST () "" "' + present_name.encode() + b'" RETURN '
        b'(STATUS (MESSAGES RECENT UNSEEN DELETED SIZE UIDNEXT UIDVALIDITY APPENDLIMIT))'
    )
    ordered = ordered_list_command(client, status_command)
    require_adjacent(
        ordered,
        f'* LIST () "/" "{present_name}"'.encode(),
        (
            f'* STATUS "{present_name}" (MESSAGES 1 RECENT 1 UNSEEN 1 DELETED 1 '
            f'SIZE {len(status_message)} UIDNEXT 2 UIDVALIDITY 1 APPENDLIMIT 26214400)'
        ).encode(),
        "STATUS for existing matching mailbox",
    )
    absent_status = ordered_list_command(
        client,
        b'LIST (SUBSCRIBED) "" "' + missing_name.encode() + b'" '
        b'RETURN (STATUS (MESSAGES))',
    )
    require_record(
        b"\n".join(absent_status),
        f'* LIST (\\NonExistent \\Subscribed) "/" "{missing_name}"'.encode(),
        "missing subscribed mailbox remains listable",
    )
    reject_record(
        b"\n".join(absent_status),
        f'* STATUS "{missing_name}"'.encode(),
        "missing mailbox has no STATUS response",
    )
    verify_utf8_status(client, suffix)
    verify_selected_recent(client, present_name)
    list_command(client, b'LIST (RECURSIVEMATCH) "" "%"', "BAD")
    client.logout()
    print("PASS imap-list-subscribed: extended LIST, UTF-8 STATUS and recursive selection")


if __name__ == "__main__":
    main()
