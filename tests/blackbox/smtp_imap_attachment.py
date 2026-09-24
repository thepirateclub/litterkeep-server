"""Independent standard-library client check for the first mail path."""
import argparse
import base64
import hashlib
import imaplib
import os
import ssl
import smtplib
from email.parser import BytesParser
from email.policy import default


PARSER = argparse.ArgumentParser()
PARSER.add_argument("host", nargs="?", default="127.0.0.1")
PARSER.add_argument("smtp_port", nargs="?", type=int, default=2525)
PARSER.add_argument("imap_port", nargs="?", type=int, default=2143)
PARSER.add_argument("--alice-password", default=os.environ.get("LITTERKEEP_ALICE_PASSWORD", "test-password"))
PARSER.add_argument("--bob-password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD", "test-password"))
OPTIONS = PARSER.parse_args()
ATTACHMENT = bytes(range(256)) + b"\x00litterkeep\xff"
ENCODED = base64.encodebytes(ATTACHMENT).replace(b"\n", b"\r\n")
MESSAGE = (
    b"From: Alice <alice@test>\r\n"
    b"To: Bob <bob@test>\r\n"
    b"Subject: attachment check\r\n"
    b"MIME-Version: 1.0\r\n"
    b"Content-Type: multipart/mixed; boundary=lk-boundary\r\n\r\n"
    b"--lk-boundary\r\nContent-Type: text/plain\r\n\r\nhello\r\n"
    b"--lk-boundary\r\n"
    b"Content-Type: application/octet-stream; name=fixture.bin\r\n"
    b"Content-Transfer-Encoding: base64\r\n"
    b"Content-Disposition: attachment; filename=fixture.bin\r\n\r\n"
    + ENCODED
    + b"--lk-boundary--\r\n"
)


def metadata_line(records):
    """Возвращает строковую часть ответа FETCH с literal-ом или без него."""
    for record in records:
        match record:
            case (response_line, _payload):
                return response_line.decode("ascii")
            case bytes():
                return record.decode("ascii")
            case _:
                continue
    raise AssertionError(f"FETCH response has no metadata: {records!r}")


TLS_CONTEXT = ssl.create_default_context()
TLS_CONTEXT.check_hostname = False
TLS_CONTEXT.verify_mode = ssl.CERT_NONE

with smtplib.SMTP(OPTIONS.host, OPTIONS.smtp_port, timeout=10) as smtp:
    smtp.starttls(context=TLS_CONTEXT)
    smtp.login("alice", OPTIONS.alice_password)
    smtp.sendmail("alice@test", ["bob@test"], MESSAGE)

with imaplib.IMAP4(OPTIONS.host, OPTIONS.imap_port, timeout=10) as imap:
    imap.starttls(ssl_context=TLS_CONTEXT)
    status, _ = imap.login("bob", OPTIONS.bob_password)
    assert status == "OK", status
    status, count = imap.select("INBOX", readonly=True)
    message_count = int(next(iter(count)))
    assert status == "OK", (status, count)
    assert message_count >= 1, message_count
    status, records = imap.fetch(
        str(message_count), "(UID FLAGS RFC822.SIZE INTERNALDATE BODY BODYSTRUCTURE RFC822)"
    )
    assert status == "OK", (status, records)
    metadata = metadata_line(records)
    for item in (
        "UID ",
        "FLAGS ",
        "RFC822.SIZE ",
        "INTERNALDATE ",
        "BODY ",
        "BODYSTRUCTURE ",
        "RFC822 {",
    ):
        assert item in metadata, (item, metadata)
    macros = (
        ("ALL", ("FLAGS ", "INTERNALDATE ", "RFC822.SIZE ", "ENVELOPE ")),
        ("FAST", ("FLAGS ", "INTERNALDATE ", "RFC822.SIZE ")),
        (
            "FULL",
            ("FLAGS ", "INTERNALDATE ", "RFC822.SIZE ", "ENVELOPE ", "BODY "),
        ),
    )
    for macro, expected_items in macros:
        status, macro_records = imap.fetch(str(message_count), macro)
        assert status == "OK", (macro, macro_records)
        macro_metadata = metadata_line(macro_records)
        for item in expected_items:
            assert item in macro_metadata, (macro, item, macro_metadata)
    status, uid_records = imap.uid("FETCH", str(message_count), "(FLAGS RFC822.SIZE)")
    assert status == "OK", uid_records
    uid_metadata = metadata_line(uid_records)
    assert "UID " in uid_metadata, uid_metadata
    assert "FLAGS " in uid_metadata and "RFC822.SIZE " in uid_metadata, uid_metadata
    raw = next(
        payload
        for record in records
        if isinstance(record, tuple)
        for _, payload in (record,)
    )

parsed = BytesParser(policy=default).parsebytes(raw)
actual = next(part.get_payload(decode=True) for part in parsed.walk() if part.get_filename() == "fixture.bin")
assert actual == ATTACHMENT, (hashlib.sha256(actual).hexdigest(), hashlib.sha256(ATTACHMENT).hexdigest())
print("PASS smtp-imap attachment sha256=" + hashlib.sha256(actual).hexdigest())
