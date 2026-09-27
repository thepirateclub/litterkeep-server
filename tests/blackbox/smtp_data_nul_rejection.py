"""Проверяет NUL refusal, SMTP recovery и отсутствие rejected message в IMAP."""

import argparse
from enum import Enum, IntEnum
from pathlib import Path
import imaplib
import smtplib
import ssl
import uuid


class SmtpReply(IntEnum):
    """Числовые SMTP-ответы независимого сетевого acceptance-теста."""

    READY = 354
    ACCEPTED = 250
    AUTHENTICATED = 235
    INVALID_CONTENT = 554
    QUIT = 221


class SmtpBodyMode(Enum):
    """Профили тела MAIL, запрещающие NUL при передаче через DATA."""

    SEVEN_BIT = ()
    EIGHT_BIT_MIME = ("BODY=8BITMIME",)


class ImapStatus(Enum):
    """Закрытые исходы IMAP-команд, полученные из ответа библиотеки."""

    OK = "OK"
    FAILED = "FAILED"

    @classmethod
    def parse(cls, value: str) -> "ImapStatus":
        """Преобразует статус IMAP в исчерпывающий исход проверки."""
        match value:
            case "OK":
                return cls.OK
            case _:
                return cls.FAILED


def password_for(path: Path, username: str) -> str:
    """Читает пароль одного bootstrap-аккаунта, не выводя секрет."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t"):
            case [account, password] if account == username:
                return password
            case _:
                continue
    raise AssertionError(f"acceptance account is absent: {username}")


def require_smtp(code: int, expected: SmtpReply, detail: bytes) -> None:
    """Сверяет код протокольного ответа и сохраняет сетевую диагностику."""
    if code != expected:
        raise AssertionError(f"expected SMTP {expected.value}, got {code}: {detail!r}")


def require_imap(status: str, operation: str, detail: object) -> None:
    """Завершает проверку ошибкой, если разобранный статус IMAP не OK."""
    if ImapStatus.parse(status) is not ImapStatus.OK:
        raise AssertionError(f"IMAP {operation} failed: {detail!r}")


def submit_nul_message(client: smtplib.SMTP, marker: str, mode: SmtpBodyMode) -> None:
    """Передаёт тело с NUL через DATA для выбранного MAIL-профиля."""
    code, detail = client.mail("alice@litterkeep.test", options=mode.value)
    require_smtp(code, SmtpReply.ACCEPTED, detail)
    code, detail = client.rcpt("bob@litterkeep.test")
    require_smtp(code, SmtpReply.ACCEPTED, detail)
    code, detail = client.docmd("DATA")
    require_smtp(code, SmtpReply.READY, detail)
    rejected = (
        f"From: alice@litterkeep.test\r\nTo: bob@litterkeep.test\r\n"
        f"Subject: rejected-{mode.name}-{marker}\r\n\r\nbody-"
    ).encode("ascii") + b"\0-octet\r\n.\r\n"
    client.send(rejected)
    code, detail = client.getreply()
    require_smtp(code, SmtpReply.INVALID_CONTENT, detail)
    if b"5.6.0" not in detail:
        raise AssertionError(f"NUL refusal omitted enhanced status: {detail!r}")


def submit_rejected_and_valid_messages(
    host: str, port: int, password: str, marker: str
) -> None:
    """Проверяет 554 5.6.0, возврат в команды и обычную доставку."""
    context = ssl._create_unverified_context()
    with smtplib.SMTP(host, port, timeout=15) as client:
        client.ehlo("rfc2045-nul-peer.test")
        client.starttls(context=context)
        client.ehlo("rfc2045-nul-peer.test")
        client.login("alice", password)
        for mode in SmtpBodyMode:
            submit_nul_message(client, marker, mode)
            code, detail = client.noop()
            require_smtp(code, SmtpReply.ACCEPTED, detail)
        valid = (
            f"From: alice@litterkeep.test\r\nTo: bob@litterkeep.test\r\n"
            f"Subject: accepted-{marker}\r\n\r\nvalid body\r\n"
        ).encode("ascii")
        refused = client.sendmail("alice@litterkeep.test", "bob@litterkeep.test", valid)
        if refused:
            raise AssertionError(f"valid post-rejection delivery failed: {refused!r}")


def require_imap_subjects(
    host: str, port: int, password: str, marker: str
) -> None:
    """Подтверждает, что в INBOX Bob сохранено только корректное письмо."""
    context = ssl._create_unverified_context()
    mailbox = imaplib.IMAP4(host, port, timeout=15)
    mailbox.starttls(ssl_context=context)
    status, detail = mailbox.login("bob", password)
    require_imap(status, "LOGIN", detail)
    try:
        status, detail = mailbox.select("INBOX", readonly=True)
        require_imap(status, "SELECT", detail)
        status, records = mailbox.search(None, "SUBJECT", f"accepted-{marker}")
        require_imap(status, "accepted message SEARCH", records)
        match records:
            case [uids]:
                accepted_count = len(uids.split())
            case _:
                raise AssertionError(f"unexpected accepted SEARCH response: {records!r}")
        if accepted_count != 1:
            raise AssertionError(f"expected one accepted message, got {accepted_count}")
        for mode in SmtpBodyMode:
            status, records = mailbox.search(
                None, "SUBJECT", f"rejected-{mode.name}-{marker}"
            )
            require_imap(status, "rejected message SEARCH", records)
            match records:
                case [uids]:
                    rejected_count = len(uids.split())
                case _:
                    raise AssertionError(f"unexpected rejected SEARCH response: {records!r}")
            if rejected_count != 0:
                raise AssertionError("NUL-containing message was stored despite SMTP refusal")
    finally:
        mailbox.logout()


def main() -> None:
    """Запускает независимую SMTP/IMAP-проверку сервера с STARTTLS."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, default=31587)
    parser.add_argument("--imap-port", type=int, default=31143)
    parser.add_argument("--credentials", required=True, type=Path)
    options = parser.parse_args()
    marker = uuid.uuid4().hex
    alice = password_for(options.credentials, "alice")
    bob = password_for(options.credentials, "bob")
    submit_rejected_and_valid_messages(options.host, options.smtp_port, alice, marker)
    require_imap_subjects(options.host, options.imap_port, bob, marker)
    print("PASS RFC 2045 DATA rejects NUL with 554 5.6.0, recovers and stores only valid mail")


if __name__ == "__main__":
    main()
