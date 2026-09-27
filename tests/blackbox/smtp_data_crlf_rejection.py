"""Проверяет отказ SMTP DATA на одиночные CR/LF и сохранность CRLF-писем."""

import argparse
from enum import Enum, IntEnum
from pathlib import Path
import imaplib
import smtplib
import ssl
import uuid


class SmtpReply(IntEnum):
    """Ожидаемые числовые ответы SMTP-сервера."""

    READY = 354
    ACCEPTED = 250
    AUTHENTICATED = 235
    INVALID_CONTENT = 554


class BodyMode(Enum):
    """Профили MIME тела, для которых DATA требует CRLF-пары."""

    SEVEN_BIT = ()
    EIGHT_BIT_MIME = ("BODY=8BITMIME",)


class LineEnding(Enum):
    """Одиночные окончания строк, недопустимые внутри MIME DATA."""

    BARE_CR = b"\r"
    BARE_LF = b"\n"


class ImapStatus(Enum):
    """Типизированный исход операции IMAP."""

    OK = "OK"
    FAILED = "FAILED"

    @classmethod
    def parse(cls, value: str) -> "ImapStatus":
        """Преобразует текст статуса библиотеки в закрытый enum."""
        match value:
            case "OK":
                return cls.OK
            case _:
                return cls.FAILED


def password_for(path: Path, username: str) -> str:
    """Читает пароль одного acceptance-аккаунта без вывода секрета."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t"):
            case [account, password] if account == username:
                return password
            case _:
                continue
    raise AssertionError(f"acceptance account is absent: {username}")


def require_smtp(code: int, expected: SmtpReply, detail: bytes) -> None:
    """Проверяет код SMTP и оставляет текст ответа в диагностике."""
    if code != expected:
        raise AssertionError(f"expected SMTP {expected.value}, got {code}: {detail!r}")


def require_imap(status: str, operation: str, detail: object) -> None:
    """Требует явный успешный результат выбранной IMAP-команды."""
    if ImapStatus.parse(status) is not ImapStatus.OK:
        raise AssertionError(f"IMAP {operation} failed: {detail!r}")


def submit_invalid_body(
    client: smtplib.SMTP,
    marker: str,
    mode: BodyMode,
    ending: LineEnding,
) -> None:
    """Передаёт одну строку с одиночным CR или LF через SMTP DATA."""
    code, detail = client.mail("alice@litterkeep.test", options=mode.value)
    require_smtp(code, SmtpReply.ACCEPTED, detail)
    code, detail = client.rcpt("bob@litterkeep.test")
    require_smtp(code, SmtpReply.ACCEPTED, detail)
    code, detail = client.docmd("DATA")
    require_smtp(code, SmtpReply.READY, detail)
    rejected = (
        f"From: alice@litterkeep.test\r\nTo: bob@litterkeep.test\r\n"
        f"Subject: rejected-{mode.name}-{ending.name}-{marker}\r\n\r\nbody-before"
    ).encode("ascii") + ending.value + b"after\r\n.\r\n"
    client.send(rejected)
    code, detail = client.getreply()
    require_smtp(code, SmtpReply.INVALID_CONTENT, detail)
    if b"5.6.0" not in detail:
        raise AssertionError(f"invalid line ending omitted enhanced status: {detail!r}")
    code, detail = client.noop()
    require_smtp(code, SmtpReply.ACCEPTED, detail)


def submit_messages(host: str, port: int, password: str, marker: str) -> None:
    """Проверяет четыре отказа и затем обычную валидную SMTP-доставку."""
    context = ssl._create_unverified_context()
    with smtplib.SMTP(host, port, timeout=15) as client:
        client.ehlo("rfc2045-crlf-peer.test")
        client.starttls(context=context)
        client.ehlo("rfc2045-crlf-peer.test")
        client.login("alice", password)
        for mode in BodyMode:
            for ending in LineEnding:
                submit_invalid_body(client, marker, mode, ending)
        valid = (
            f"From: alice@litterkeep.test\r\nTo: bob@litterkeep.test\r\n"
            f"Subject: accepted-{marker}\r\n\r\ncanonical CRLF body\r\n"
        ).encode("ascii")
        refused = client.sendmail("alice@litterkeep.test", "bob@litterkeep.test", valid)
        if refused:
            raise AssertionError(f"valid follow-up delivery failed: {refused!r}")


def verify_mailbox(host: str, port: int, password: str, marker: str) -> None:
    """Подтверждает принятие валидного письма и отсутствие четырёх отказов."""
    mailbox = imaplib.IMAP4(host, port, timeout=15)
    mailbox.starttls(ssl_context=ssl._create_unverified_context())
    status, detail = mailbox.login("bob", password)
    require_imap(status, "LOGIN", detail)
    try:
        status, detail = mailbox.select("INBOX", readonly=True)
        require_imap(status, "SELECT", detail)
        status, records = mailbox.search(None, "SUBJECT", f"accepted-{marker}")
        require_imap(status, "valid message SEARCH", records)
        match records:
            case [uids]:
                accepted_count = len(uids.split())
            case _:
                raise AssertionError(f"unexpected valid SEARCH response: {records!r}")
        if accepted_count != 1:
            raise AssertionError(f"expected one valid message, got {accepted_count}")
        for mode in BodyMode:
            for ending in LineEnding:
                subject = f"rejected-{mode.name}-{ending.name}-{marker}"
                status, records = mailbox.search(None, "SUBJECT", subject)
                require_imap(status, "rejected message SEARCH", records)
                match records:
                    case [uids]:
                        rejected_count = len(uids.split())
                    case _:
                        raise AssertionError(f"unexpected rejected SEARCH response: {records!r}")
                if rejected_count != 0:
                    raise AssertionError(f"invalid line ending was stored: {subject}")
    finally:
        mailbox.logout()


def main() -> None:
    """Запускает независимую SMTP/IMAP acceptance-проверку STARTTLS."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, default=31587)
    parser.add_argument("--imap-port", type=int, default=31143)
    parser.add_argument("--credentials", required=True, type=Path)
    options = parser.parse_args()
    marker = uuid.uuid4().hex
    alice = password_for(options.credentials, "alice")
    bob = password_for(options.credentials, "bob")
    submit_messages(options.host, options.smtp_port, alice, marker)
    verify_mailbox(options.host, options.imap_port, bob, marker)
    print("PASS SMTP DATA rejects bare CR/LF for 7BIT and 8BITMIME, preserves canonical CRLF")


if __name__ == "__main__":
    main()
