"""Независимо читает DSN из INBOX локального envelope sender через IMAP STARTTLS."""

import argparse
from enum import Enum
import imaplib
import ssl


class TestAccount(Enum):
    """Имя аккаунта, чей INBOX используется acceptance-сценарием."""

    ALICE = "alice"


def password_for(path: str, account: TestAccount) -> str:
    """Читает локальный пароль, созданный вместе с тестовым Store."""
    with open(path, encoding="utf-8") as credentials_file:
        for line in credentials_file:
            match line.rstrip("\n").split("\t", maxsplit=1):
                case [name, password] if name == account.value:
                    return password
    raise AssertionError(f"credential fixture has no {account.value} account")


class ImapStatus(Enum):
    """Типизированные IMAP statuses, существенные для сценария."""

    OK = "OK"


def parser() -> argparse.ArgumentParser:
    """Определяет только параметры независимого стандартного клиента."""
    result = argparse.ArgumentParser()
    result.add_argument("--host", default="127.0.0.1")
    result.add_argument("--port", type=int, required=True)
    result.add_argument("--ca", required=True)
    result.add_argument("--credentials", required=True)
    return result


def require_dsn(records: list[object]) -> None:
    """Ищет DSN, не полагаясь на parser или encoder нашего сервера."""
    for record in records:
        match record:
            case (_, raw) if isinstance(raw, bytes):
                required = (
                    b"report-type=delivery-status",
                    b"Action: failed",
                    b"Final-Recipient: rfc822; remote@peer.test",
                    b"Subject: queue redirect peer",
                )
                if all(marker in raw for marker in required):
                    return
            case _:
                pass
    raise AssertionError("delivery status notification is absent")


def main() -> None:
    """Выполняет TLS login и читает сохранённый DSN из Alice INBOX."""
    options = parser().parse_args()
    context = ssl.create_default_context(cafile=options.ca)
    alice_password = password_for(options.credentials, TestAccount.ALICE)
    client = imaplib.IMAP4(options.host, options.port, timeout=10)
    try:
        status, _ = client.starttls(ssl_context=context)
        assert ImapStatus(status) is ImapStatus.OK, status
        status, _ = client.login("alice", alice_password)
        assert ImapStatus(status) is ImapStatus.OK, status
        status, counts = client.select("INBOX", readonly=True)
        assert ImapStatus(status) is ImapStatus.OK, status
        total = int(next(iter(counts)))
        assert total >= 1, "Alice INBOX is empty"
        status, records = client.fetch(f"1:{total}", "(RFC822)")
        assert ImapStatus(status) is ImapStatus.OK, status
        require_dsn(records)
    finally:
        client.shutdown()
    print("queue DSN read: PASS")


if __name__ == "__main__":
    main()
