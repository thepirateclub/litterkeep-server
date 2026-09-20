"""Независимая IMAP/MIME/SHA-256 проверка полного seed manifest."""

from __future__ import annotations

import argparse
import csv
import hashlib
import imaplib
import socket
import ssl
from collections import defaultdict
from email import policy
from email.parser import BytesParser
from pathlib import Path


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", required=True)
    parser.add_argument("--address", required=True)
    parser.add_argument("--port", required=True, type=int)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--alice-password", required=True)
    parser.add_argument("--bob-password", required=True)
    parser.add_argument("--qa-password", required=True)
    return parser.parse_args()


def grouped_manifest(path: str) -> dict[str, list[dict[str, str]]]:
    grouped: dict[str, list[dict[str, str]]] = defaultdict(list)
    with Path(path).open(encoding="utf-8", newline="") as source:
        for row in csv.DictReader(source, delimiter="\t"):
            grouped[row["fixture_id"]].append(row)
    return dict(grouped)


def connect(config: argparse.Namespace, owner: str) -> imaplib.IMAP4_SSL:
    der = Path(config.ca).read_bytes()
    context = ssl.create_default_context(cadata=ssl.DER_cert_to_PEM_cert(der))
    original = socket.getaddrinfo

    def local_address(host: str, port: int, *values: object, **named: object):
        target = config.address if host == config.host else host
        return original(target, port, *values, **named)

    socket.getaddrinfo = local_address
    try:
        client = imaplib.IMAP4_SSL(config.host, config.port, ssl_context=context)
    finally:
        socket.getaddrinfo = original
    passwords = {
        "alice": config.alice_password,
        "bob": config.bob_password,
        "qa": config.qa_password,
    }
    password = passwords[owner]
    status, _ = client.login(owner, password)
    require(status == "OK", f"LOGIN {owner}")
    return client


def fixture_message(client: imaplib.IMAP4_SSL, mailbox: str, fixture_id: str) -> bytes:
    status, _ = client.select(mailbox, readonly=True)
    require(status == "OK", f"SELECT {mailbox}")
    status, search = client.search(None, "HEADER", "X-Litterkeep-Fixture", fixture_id)
    require(status == "OK", f"SEARCH {fixture_id}")
    ids = next(iter(search), b"").split()
    require(len(ids) == 1, f"fixture count {fixture_id}: {len(ids)}")
    sequence = next(iter(ids))
    status, fetched = client.fetch(sequence, "(RFC822)")
    require(status == "OK", f"FETCH {fixture_id}")
    for value in fetched:
        if isinstance(value, tuple):
            _, body = value
            return body
    raise AssertionError(f"missing RFC822 {fixture_id}")


def verify_attachments(raw: bytes, rows: list[dict[str, str]]) -> int:
    expected = [row for row in rows if row["decoded_sha256"] != "-"]
    message = BytesParser(policy=policy.default).parsebytes(raw)
    actual = [part for part in message.walk() if part.get_content_disposition() == "attachment"]
    first_row = next(iter(rows))
    require(len(actual) == len(expected), f"attachment count for {first_row['fixture_id']}")
    checked = 0
    for part, row in zip(actual, expected, strict=True):
        payload = part.get_payload(decode=True)
        require(payload is not None, "attachment payload")
        require(part.get_filename() == row["attachment_name"], "attachment filename")
        require(len(payload) == int(row["decoded_size"]), "attachment size")
        require(hashlib.sha256(payload).hexdigest() == row["decoded_sha256"], "attachment sha256")
        checked += 1
    return checked


def require(condition: bool, context: str) -> None:
    if not condition:
        raise AssertionError(context)


def main() -> None:
    config = arguments()
    manifest = grouped_manifest(config.manifest)
    clients: dict[str, imaplib.IMAP4_SSL] = {}
    attachments = 0
    try:
        for fixture_id, rows in manifest.items():
            row = next(iter(rows))
            first = True
            for owner in row["owner"].split("+"):
                if owner not in clients:
                    clients[owner] = connect(config, owner)
                client = clients[owner]
                raw = fixture_message(client, row["mailbox"], fixture_id)
                if first:
                    attachments += verify_attachments(raw, rows)
                    first = False
    finally:
        for client in clients.values():
            client.logout()
    print(f"MANIFEST ok fixtures={len(manifest)} attachments={attachments}")


if __name__ == "__main__":
    main()
