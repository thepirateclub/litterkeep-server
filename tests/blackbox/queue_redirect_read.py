"""Подтверждает redirect-only delivery независимым стандартным IMAP-клиентом."""

import argparse
from enum import Enum
import imaplib


class ImapStatus(Enum):
    """Статусы ответов, существенные для acceptance-критерия."""

    OK = "OK"


def parser() -> argparse.ArgumentParser:
    """Строит typed границу параметров независимого клиента."""
    result = argparse.ArgumentParser()
    result.add_argument("--host", required=True)
    result.add_argument("--port", type=int, required=True)
    return result


def assert_delivered(client: imaplib.IMAP4) -> None:
    """Ищет исходное письмо и подтверждает, что локальная копия отсутствует."""
    status, counts = client.select("INBOX", readonly=True)
    assert ImapStatus(status) is ImapStatus.OK, status
    total = int(next(iter(counts)))
    assert total == 1, f"expected one remote message, got {total}"
    status, records = client.fetch("1", "(RFC822)")
    assert ImapStatus(status) is ImapStatus.OK, status
    for record in records:
        match record:
            case (_, raw) if b"Subject: queue redirect peer" in raw:
                return
            case _:
                pass
    raise AssertionError("redirected message is absent")


def main() -> None:
    """Логинится как внешний получатель и запускает проверку mailbox."""
    options = parser().parse_args()
    client = imaplib.IMAP4(options.host, options.port, timeout=10)
    try:
        status, _ = client.login("remote", "remote-password")
        assert ImapStatus(status) is ImapStatus.OK, status
        assert_delivered(client)
    finally:
        client.shutdown()
    print("queue redirect read: PASS")


if __name__ == "__main__":
    main()
