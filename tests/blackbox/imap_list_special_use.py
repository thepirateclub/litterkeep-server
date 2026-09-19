"""Проверяет LIST wildcard и extended SPECIAL-USE через STARTTLS."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    return parser.parse_args()


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


def require_line(records: object, value: bytes) -> None:
    if not isinstance(records, list):
        raise AssertionError(f"unexpected LIST data: {records!r}")
    for record in records:
        if isinstance(record, bytes):
            if value in record:
                return
    raise AssertionError(f"LIST line absent: {value!r} in {records!r}")


def reject_line(records: object, value: bytes) -> None:
    if not isinstance(records, list):
        raise AssertionError(f"unexpected LIST data: {records!r}")
    for record in records:
        if isinstance(record, bytes):
            if value in record:
                raise AssertionError(f"unexpected LIST line: {record!r}")


def special_use_list(client: imaplib.IMAP4) -> object:
    tag = client._new_tag()
    client.send(tag + b' LIST (SPECIAL-USE) "" "*"\r\n')
    require(client._get_tagged_response(tag), "OK")
    return client.untagged_responses.pop("LIST", [])


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port)
    client.starttls(ssl_context=context)
    require(client.login("bob", "bob-password"), "OK")

    capability = require(client.capability(), "OK")
    if not isinstance(capability, list):
        raise AssertionError(f"unexpected capability data: {capability!r}")
    require_line(capability, b"SPECIAL-USE")

    wildcard = require(client.list("", "Invoices/%"), "OK")
    require_line(wildcard, b"Invoices/2026")

    special = special_use_list(client)
    require_line(special, b"\\Sent")
    require_line(special, b"\\Drafts")
    reject_line(special, b"Invoices/2026")
    client.logout()
    print("IMAP LIST SPECIAL-USE passed: wildcard and selection are distinct")


if __name__ == "__main__":
    main()
