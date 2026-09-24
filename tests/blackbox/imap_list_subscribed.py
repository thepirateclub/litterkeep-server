"""Проверяет RFC LIST (SUBSCRIBED) через независимый TLS IMAP-клиент."""

import argparse
import imaplib
import os
import ssl
import uuid


def require(status: str, detail: object, expected: str, operation: str) -> object:
    if status != expected:
        raise AssertionError(f"{operation}: {status} {detail!r}")
    return detail


def list_command(client: imaplib.IMAP4, command: bytes, expected: str = "OK") -> bytes:
    tagged_command(client, command, expected)
    values = client.untagged_responses.pop("LIST", [])
    return b"\n".join(value for value in values if isinstance(value, bytes))


def tagged_command(client: imaplib.IMAP4, command: bytes, expected: str) -> None:
    tag = client._new_tag()
    client.send(tag + b" " + command + b"\r\n")
    status, detail = client._get_tagged_response(tag)
    require(status, detail, expected, command.decode("ascii"))


def require_record(records: bytes, expected: bytes, operation: str) -> None:
    if expected not in records:
        raise AssertionError(f"{operation}: expected {expected!r} in {records!r}")


def reject_record(records: bytes, unexpected: bytes, operation: str) -> None:
    if unexpected in records:
        raise AssertionError(f"{operation}: unexpected {unexpected!r} in {records!r}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("host", nargs="?", default="127.0.0.1")
    parser.add_argument("port", nargs="?", type=int, default=2149)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    options = parser.parse_args()
    trust = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4(options.host, options.port, timeout=10)
    client.starttls(ssl_context=trust)
    password = os.environ["LITTERKEEP_BOB_PASSWORD"]
    require(*client.login("bob", password), "OK", "LOGIN")
    suffix = uuid.uuid4().hex
    present_name = f"SubscribedPresent{suffix}"
    missing_name = f"SubscribedMissing{suffix}"
    child_name = f"{present_name}/Nested"
    phantom_child = f"Phantom{suffix}/Nested"
    require(*client.create(present_name), "OK", "CREATE")
    require(*client.create(child_name), "OK", "CREATE nested")
    require(*client.subscribe(present_name), "OK", "SUBSCRIBE existing")
    require(*client.subscribe(missing_name), "OK", "SUBSCRIBE missing")
    require(*client.subscribe(child_name), "OK", "SUBSCRIBE nested")
    require(*client.subscribe(phantom_child), "OK", "SUBSCRIBE phantom child")
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
    list_command(client, b'LIST (RECURSIVEMATCH) "" "%"', "BAD")
    client.logout()
    print("PASS imap-list-subscribed: recursive parents, CHILDINFO and invalid option")


if __name__ == "__main__":
    main()
