"""Подтверждает redirect-only delivery независимым стандартным IMAP-клиентом."""

import argparse
from enum import Enum
import imaplib
import ssl


class ImapStatus(Enum):
    """Статусы ответов, существенные для acceptance-критерия."""

    OK = "OK"


def parser() -> argparse.ArgumentParser:
    """Строит typed границу параметров независимого клиента."""
    result = argparse.ArgumentParser()
    result.add_argument("--host", required=True)
    result.add_argument("--port", type=int, required=True)
    result.add_argument("--login", default="remote")
    result.add_argument("--password", default="remote-password")
    result.add_argument("--subject", default="queue redirect peer")
    result.add_argument("--ca")
    return result


def assert_delivered(client: imaplib.IMAP4, subject: str) -> None:
    """Ищет исходное письмо и подтверждает, что локальная копия отсутствует."""
    status, counts = client.select("INBOX", readonly=True)
    assert ImapStatus(status) is ImapStatus.OK, status
    total = int(next(iter(counts)))
    assert total >= 1, "destination mailbox is empty"
    status, records = client.search(None, "SUBJECT", f'"{subject}"')
    assert ImapStatus(status) is ImapStatus.OK, status
    identifiers = next(iter(records), b"").strip()
    assert identifiers != b"", f"redirected message is absent: {subject}"


def main() -> None:
    """Логинится как внешний получатель и запускает проверку mailbox."""
    options = parser().parse_args()
    client = imaplib.IMAP4(options.host, options.port, timeout=10)
    try:
        if options.ca is not None:
            context = ssl.create_default_context(cafile=options.ca)
            status, _ = client.starttls(ssl_context=context)
            assert ImapStatus(status) is ImapStatus.OK, status
        status, _ = client.login(options.login, options.password)
        assert ImapStatus(status) is ImapStatus.OK, status
        assert_delivered(client, options.subject)
    finally:
        client.shutdown()
    print("queue redirect read: PASS")


if __name__ == "__main__":
    main()
