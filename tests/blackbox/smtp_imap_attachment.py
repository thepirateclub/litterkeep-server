"""Independent standard-library client check for the first mail path."""
import base64
import hashlib
import imaplib
import smtplib
import sys
from email.parser import BytesParser
from email.policy import default


HOST = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1"
SMTP_PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 2525
IMAP_PORT = int(sys.argv[3]) if len(sys.argv) > 3 else 2143
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

with smtplib.SMTP(HOST, SMTP_PORT, timeout=10) as smtp:
    smtp.sendmail("alice@test", ["bob@test"], MESSAGE)

with imaplib.IMAP4(HOST, IMAP_PORT, timeout=10) as imap:
    status, _ = imap.login("bob", "test-password")
    assert status == "OK", status
    status, count = imap.select("INBOX", readonly=True)
    message_count = int(next(iter(count)))
    assert status == "OK", (status, count)
    assert message_count >= 1, message_count
    status, records = imap.fetch(str(message_count), "(RFC822)")
    assert status == "OK", (status, records)
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
