"""Проверяет через стандартный IMAP-клиент факт доставки исходящего fixture."""
from pathlib import Path
from enum import Enum
import imaplib
import sys


class ImapStatus(Enum):
    """Статусы, возвращаемые независимым стандартным IMAP-клиентом."""

    OK = "OK"


HOST = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 2143
FIXTURE = Path(__file__).parent.parent / "fixtures" / "outbound.eml"

with imaplib.IMAP4(HOST, PORT, timeout=10) as client:
    status, _ = client.login("bob", "test-password")
    assert ImapStatus(status) is ImapStatus.OK, status
    status, counts = client.select("INBOX", readonly=True)
    assert ImapStatus(status) is ImapStatus.OK, status
    total = int(next(iter(counts)))
    actual = None
    for number in range(1, total + 1):
        status, records = client.fetch(str(number), "(RFC822)")
        match ImapStatus(status):
            case ImapStatus.OK:
                pass
        for record in records:
            match record:
                case (_, raw) if b"Subject: outbound transport check" in raw:
                    actual = raw
                case _:
                    pass

assert actual is not None
assert actual.rstrip(b"\r\n") == FIXTURE.read_bytes().rstrip(b"\r\n"), actual
print("PASS outbound fixture is readable through IMAP")
