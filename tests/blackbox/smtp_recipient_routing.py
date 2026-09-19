"""Проверяет, что SMTP envelope задаёт mailbox, а не фиксированное имя."""
import imaplib
import smtplib
import sys


HOST = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1"
SMTP_PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 2525
IMAP_PORT = int(sys.argv[3]) if len(sys.argv) > 3 else 2143
MESSAGE = b"From: sender@test\r\nTo: alice@test\r\nSubject: route\r\n\r\nrecipient-owned\r\n"

with smtplib.SMTP(HOST, SMTP_PORT, timeout=10) as smtp:
    smtp.sendmail("sender@test", ["alice@test"], MESSAGE)

with smtplib.SMTP(HOST, SMTP_PORT, timeout=10) as smtp:
    smtp.mail("sender@test")
    code, _ = smtp.rcpt("unknown@test")
    assert code == 550, code

with imaplib.IMAP4(HOST, IMAP_PORT, timeout=10) as imap:
    status, _ = imap.login("alice", "alice-password")
    assert status == "OK", status
    status, count = imap.select("INBOX", readonly=True)
    assert status == "OK", (status, count)
    message_count = int(next(iter(count)))
    assert message_count == 1, message_count
    status, records = imap.fetch("1", "(RFC822)")
    assert status == "OK", (status, records)
    received = next(
        payload
        for record in records
        if isinstance(record, tuple)
        for _, payload in (record,)
    )

assert received == MESSAGE, received
print("PASS smtp-recipient-routing")
