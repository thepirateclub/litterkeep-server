"""Проверяет через стандартный IMAP-клиент факт доставки исходящего fixture."""
from pathlib import Path
import imaplib
import sys


HOST = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 2143
FIXTURE = Path(__file__).parent.parent / "fixtures" / "outbound.eml"

with imaplib.IMAP4(HOST, PORT, timeout=10) as client:
    status, _ = client.login("bob", "test-password")
    assert status == "OK", status
    status, counts = client.select("INBOX", readonly=True)
    assert status == "OK", status
    total = int(next(iter(counts)))
    actual = next(
        part[1]
        for number in range(1, total + 1)
        for status, records in [client.fetch(str(number), "(RFC822)")]
        if status == "OK"
        for part in records
        if isinstance(part, tuple)
        if b"Subject: outbound transport check" in part[1]
    )

assert actual.rstrip(b"\r\n") == FIXTURE.read_bytes().rstrip(b"\r\n"), actual
print("PASS outbound fixture is readable through IMAP")
