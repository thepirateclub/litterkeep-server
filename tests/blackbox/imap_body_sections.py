"""Независимая проверка RFC BODY sections и флага Seen через STARTTLS."""
import argparse
import email
import hashlib
import imaplib
import os
import smtplib
import ssl
from email.message import EmailMessage
from email.policy import SMTP


PARSER = argparse.ArgumentParser()
PARSER.add_argument("host", nargs="?", default="127.0.0.1")
PARSER.add_argument("smtp_port", nargs="?", type=int, default=2525)
PARSER.add_argument("imap_port", nargs="?", type=int, default=2143)
PARSER.add_argument("--verify-seen-restart", action="store_true")
OPTIONS = PARSER.parse_args()
ALICE_PASSWORD = os.environ.get("LITTERKEEP_ALICE_PASSWORD", "test-password")
BOB_PASSWORD = os.environ.get("LITTERKEEP_BOB_PASSWORD", "test-password")
ATTACHMENT = bytes(range(256)) + b"\x00imap-sections\xff"
SUBJECT = "body section acceptance " + os.urandom(8).hex()
MESSAGE = EmailMessage()
MESSAGE["From"] = "Alice <alice@test>"
MESSAGE["To"] = "Bob <bob@test>"
MESSAGE["Subject"] = SUBJECT
MESSAGE.set_content("plain body payload")
MESSAGE.add_attachment(ATTACHMENT, maintype="application", subtype="octet-stream", filename="fixture.bin")
MESSAGE.set_boundary("g14-body-sections")
RAW_MESSAGE = MESSAGE.as_bytes(policy=SMTP)

TLS_CONTEXT = ssl.create_default_context()
TLS_CONTEXT.check_hostname = False
TLS_CONTEXT.verify_mode = ssl.CERT_NONE


def literal_payload(records):
    """Извлекает первый literal без позиционного доступа к коллекции."""
    for record in records:
        match record:
            case (_metadata, payload) if isinstance(payload, bytes):
                return payload
            case _:
                continue
    raise AssertionError(f"FETCH response has no literal: {records!r}")


def metadata(records):
    """Извлекает metadata строку ответа FETCH."""
    for record in records:
        match record:
            case (line, _payload):
                return line.decode("ascii")
            case bytes():
                return record.decode("ascii")
            case _:
                continue
    raise AssertionError(f"FETCH response has no metadata: {records!r}")


if OPTIONS.verify_seen_restart:
    with imaplib.IMAP4(OPTIONS.host, OPTIONS.imap_port, timeout=10) as imap:
        imap.starttls(ssl_context=TLS_CONTEXT)
        status, _ = imap.login("bob", BOB_PASSWORD)
        assert status == "OK", status
        status, _ = imap.select("INBOX", readonly=True)
        assert status == "OK", status
        status, matches = imap.search(None, "HEADER", "SUBJECT", '"body section acceptance"')
        assert status == "OK", (status, matches)
        sequences = next(iter(matches)).decode("ascii").split()
        seen_after_restart = False
        for sequence in sequences:
            status, flags = imap.fetch(sequence, "(FLAGS)")
            assert status == "OK", (status, flags)
            seen_after_restart = seen_after_restart or "\\Seen" in metadata(flags)
        assert seen_after_restart, "durable Seen flag was lost after server restart"
    print("PASS imap-seen-survives-restart")
    raise SystemExit(0)

with smtplib.SMTP(OPTIONS.host, OPTIONS.smtp_port, timeout=10) as smtp:
    smtp.starttls(context=TLS_CONTEXT)
    smtp.login("alice", ALICE_PASSWORD)
    smtp.sendmail("alice@test", ["bob@test"], RAW_MESSAGE)

with imaplib.IMAP4(OPTIONS.host, OPTIONS.imap_port, timeout=10) as imap:
    imap.starttls(ssl_context=TLS_CONTEXT)
    status, _ = imap.login("bob", BOB_PASSWORD)
    assert status == "OK", status
    status, _ = imap.select("INBOX")
    assert status == "OK", status
    status, search = imap.search(None, "HEADER", "SUBJECT", f'"{SUBJECT}"')
    assert status == "OK", (status, search)
    uid_text = next(iter(search)).decode("ascii").split()
    message_number = next(iter(uid_text))

    status, peek = imap.fetch(message_number, "(BODY.PEEK[HEADER.FIELDS (subject)] BODY.PEEK[TEXT])")
    assert status == "OK", (status, peek)
    peek_header = literal_payload(peek)
    assert peek_header == f"Subject: {SUBJECT}\r\n\r\n".encode("ascii"), peek_header
    assert "BODY[HEADER.FIELDS (subject)]" in metadata(peek), metadata(peek)
    status, flags = imap.fetch(message_number, "(FLAGS)")
    assert status == "OK", (status, flags)
    assert "\\Seen" not in metadata(flags), metadata(flags)

    status, not_subject = imap.fetch(message_number, "(BODY.PEEK[HEADER.FIELDS.NOT (subject)])")
    assert status == "OK", (status, not_subject)
    excluded_header = literal_payload(not_subject)
    assert b"Subject:" not in excluded_header, excluded_header
    assert b"From:" in excluded_header, excluded_header

    status, raw_header = imap.fetch(message_number, "(BODY.PEEK[HEADER])")
    assert status == "OK", (status, raw_header)
    header = literal_payload(raw_header)
    assert header.endswith(b"\r\n\r\n"), header

    status, _ = imap.store(message_number, "-FLAGS.SILENT", "(\\Seen)")
    assert status == "OK", status
    status, legacy_header = imap.fetch(message_number, "(RFC822.HEADER)")
    assert status == "OK", (status, legacy_header)
    legacy_header_bytes = literal_payload(legacy_header)
    expected_header = RAW_MESSAGE.partition(b"\r\n\r\n")[0] + b"\r\n\r\n"
    assert legacy_header_bytes.endswith(expected_header), legacy_header_bytes
    assert "RFC822.HEADER" in metadata(legacy_header), metadata(legacy_header)
    status, flags = imap.fetch(message_number, "(FLAGS)")
    assert status == "OK", (status, flags)
    assert "\\Seen" not in metadata(flags), metadata(flags)

    status, legacy_text = imap.fetch(message_number, "(RFC822.TEXT)")
    assert status == "OK", (status, legacy_text)
    legacy_text_bytes = literal_payload(legacy_text)
    expected_text = RAW_MESSAGE.partition(b"\r\n\r\n")[2]
    assert legacy_text_bytes == expected_text, legacy_text_bytes
    assert "RFC822.TEXT" in metadata(legacy_text), metadata(legacy_text)
    status, flags = imap.fetch(message_number, "(FLAGS)")
    assert status == "OK", (status, flags)
    assert "\\Seen" in metadata(flags), metadata(flags)
    status, _ = imap.store(message_number, "-FLAGS.SILENT", "(\\Seen)")
    assert status == "OK", status

    status, body_text = imap.fetch(message_number, "(BODY.PEEK[TEXT])")
    assert status == "OK", (status, body_text)
    parsed = email.message_from_bytes(RAW_MESSAGE)
    expected = next(part.get_payload(decode=True) for part in parsed.walk() if part.get_filename() == "fixture.bin")
    raw = literal_payload(body_text)
    root_headers, _separator, _source_body = RAW_MESSAGE.partition(b"\r\n\r\n")
    decoded = email.message_from_bytes(root_headers + b"\r\n\r\n" + raw)
    actual = next(part.get_payload(decode=True) for part in decoded.walk() if part.get_filename() == "fixture.bin")
    assert actual == ATTACHMENT, (hashlib.sha256(actual).hexdigest(), hashlib.sha256(ATTACHMENT).hexdigest())

    status, body = imap.fetch(message_number, "(BODY[TEXT])")
    assert status == "OK", (status, body)
    assert "BODY[TEXT]" in metadata(body), metadata(body)
    status, flags = imap.fetch(message_number, "(FLAGS)")
    assert status == "OK", (status, flags)
    assert "\\Seen" in metadata(flags), metadata(flags)

print("PASS imap-body-sections attachment-sha256=" + hashlib.sha256(ATTACHMENT).hexdigest())
