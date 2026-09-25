"""Проверяет MIME multipart boundary padding через независимый IMAP-клиент."""

import argparse
import base64
import imaplib
import os
import ssl
import uuid


def options() -> argparse.Namespace:
    """Читает только адрес listener-а и локальные данные TLS/аккаунта."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="localhost")
    parser.add_argument("--imap-port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument(
        "--password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD")
    )
    return parser.parse_args()


def fixture() -> tuple[bytes, bytes, bytes, str]:
    """Строит multipart с transport-padding и boundary-похожим текстом."""
    boundary = ("g17-" + uuid.uuid4().hex).encode("ascii")
    subject = "multipart padding " + uuid.uuid4().hex
    attachment = bytes(range(256)) + b"\x00padding\xff"
    encoded = base64.encodebytes(attachment).replace(b"\n", b"\r\n")
    raw = (
        b"From: alice@test\r\nTo: bob@test\r\nSubject: "
        + subject.encode("ascii")
        + b"\r\nContent-Type: multipart/mixed; boundary=\""
        + boundary
        + b"\"\r\n\r\nexplanatory preamble\r\n--"
        + boundary
        + b" \t\r\nContent-Type: text/plain; charset=utf-8\r\n"
        + b"Content-Transfer-Encoding: 7bit\r\n\r\nfirst line\r\nlast line\r\n--"
        + boundary
        + b"\t\r\nContent-Type: application/octet-stream\r\n"
        + b"Content-Transfer-Encoding: base64\r\n\r\n"
        + encoded
        + b"--"
        + boundary
        + b"-- \t\r\nepilogue\r\n"
    )
    expected_text = b"first line\r\nlast line"
    return raw, expected_text, attachment, subject


def literal(records: list[object]) -> bytes:
    """Извлекает literal FETCH без предположений о его позиции."""
    for record in records:
        match record:
            case (_metadata, payload) if isinstance(payload, bytes):
                return payload
            case _:
                continue
    raise AssertionError(f"FETCH literal is absent: {records!r}")


def main() -> None:
    selected = options()
    if selected.password is None:
        raise ValueError("provide --password or LITTERKEEP_BOB_PASSWORD")
    raw, expected_text, expected_attachment, subject = fixture()

    context = ssl.create_default_context(cafile=selected.ca)
    context.check_hostname = False
    with imaplib.IMAP4(selected.host, selected.imap_port, timeout=10) as client:
        client.starttls(ssl_context=context)
        status, _ = client.login("bob", selected.password)
        if status != "OK":
            raise AssertionError("IMAP authentication failed")
        status, _ = client.append("INBOX", None, None, raw)
        if status != "OK":
            raise AssertionError(f"IMAP APPEND failed: {status!r}")
        status, _ = client.select("INBOX", readonly=True)
        if status != "OK":
            raise AssertionError("INBOX selection failed")
        status, matches = client.search(None, "HEADER", "SUBJECT", f'"{subject}"')
        if status != "OK":
            raise AssertionError("message search failed")
        sequence = next(iter(matches)).decode("ascii").split()
        message_number = next(iter(sequence))

        status, structure = client.fetch(message_number, "(BODYSTRUCTURE)")
        if status != "OK":
            raise AssertionError(f"BODYSTRUCTURE failed: {status!r} {structure!r}")
        wire_structure = b" ".join(
            item for item in structure if isinstance(item, bytes)
        )
        for required in (
            b'"MIXED"',
            b'"TEXT" "PLAIN"',
            b'"APPLICATION" "OCTET-STREAM"',
        ):
            if required not in wire_structure:
                raise AssertionError(f"BODYSTRUCTURE lacks {required!r}: {wire_structure!r}")
        status, first_part = client.fetch(message_number, "(BODY.PEEK[1])")
        if status != "OK" or literal(first_part) != expected_text:
            raise AssertionError("boundary padding shifted or truncated the first part")
        status, binary_part = client.fetch(message_number, "(BINARY.PEEK[2])")
        if status != "OK" or literal(binary_part) != expected_attachment:
            raise AssertionError("padded closing boundary changed decoded attachment")

    print("PASS imap-multipart-transport-padding: body ranges and attachment bytes")


if __name__ == "__main__":
    main()
