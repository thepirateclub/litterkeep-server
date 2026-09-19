"""Проверяет, что SMTP envelope задаёт mailbox, а не фиксированное имя."""
import argparse
import imaplib
import smtplib


PARSER = argparse.ArgumentParser()
PARSER.add_argument("host", nargs="?", default="127.0.0.1")
PARSER.add_argument("smtp_port", nargs="?", type=int, default=2525)
PARSER.add_argument("imap_port", nargs="?", type=int, default=2143)
OPTIONS = PARSER.parse_args()
MESSAGE = b"From: sender@test\r\nTo: alice@test\r\nSubject: route\r\n\r\nrecipient-owned\r\n"

with smtplib.SMTP(OPTIONS.host, OPTIONS.smtp_port, timeout=10) as smtp:
    smtp.sendmail("sender@test", ["alice@test"], MESSAGE)

with smtplib.SMTP(OPTIONS.host, OPTIONS.smtp_port, timeout=10) as smtp:
    smtp.mail("sender@test")
    code, _ = smtp.rcpt("unknown@test")
    assert code == 550, code

with smtplib.SMTP(OPTIONS.host, OPTIONS.smtp_port, timeout=10) as smtp:
    smtp.mail("sender@test")
    code, _ = smtp.rcpt("alice@outside.test")
    assert code == 550, code

with imaplib.IMAP4(OPTIONS.host, OPTIONS.imap_port, timeout=10) as imap:
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
