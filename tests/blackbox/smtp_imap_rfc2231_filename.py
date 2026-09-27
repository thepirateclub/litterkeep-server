"""Проверяет RFC 2231 filename continuations через SMTP и IMAP."""

from __future__ import annotations

import argparse
import imaplib
import smtplib
import ssl
from enum import Enum
from pathlib import Path


class ImapStatus(Enum):
    """Типизированные исходы IMAP-команд этого acceptance-сценария."""

    OK = "OK"
    FAILED = "FAILED"

    @classmethod
    def parse(cls, value: str) -> ImapStatus:
        """Сопоставляет ответ библиотеки с закрытым набором исходов."""
        match value:
            case "OK":
                return cls.OK
            case _:
                return cls.FAILED


def password_for(credentials: Path, username: str) -> str:
    """Читает пароль из локального acceptance fixture без argv-секрета."""
    for line in credentials.read_text(encoding="utf-8").splitlines():
        fields = line.split("\t", maxsplit=1)
        match fields:
            case [name, password] if name == username:
                return password
            case _:
                continue
    raise AssertionError(f"аккаунт {username!r} отсутствует в fixture")


def message_bytes(marker: str) -> bytes:
    """Собирает письмо с language-tag и сегментированным RFC 2231 filename."""
    subject = f"RFC2231 filename {marker}"
    return (
        b"From: Alice <alice@litterkeep.test>\r\n"
        b"To: Bob <bob@litterkeep.test>\r\n"
        + f"Subject: {subject}\r\n".encode("ascii")
        + b"MIME-Version: 1.0\r\n"
        + b"Content-Type: application/octet-stream\r\n"
        + b"Content-Transfer-Encoding: base64\r\n"
        + b"Content-Disposition: attachment;\r\n"
        + b" filename*0*=UTF-8'en-US'%F0%9F%98%80%20;\r\n"
        + b" filename*1*=r%C3%A9sum%C3%A9.txt\r\n"
        + b"\r\nAAEC/w==\r\n"
    )


def fetch_message(imap: imaplib.IMAP4, subject: str) -> bytes:
    """Находит единственное письмо и получает структуру с исходными headers."""
    status, matches = imap.search(None, "SUBJECT", f'"{subject}"')
    if ImapStatus.parse(status) is not ImapStatus.OK:
        raise AssertionError(f"IMAP SEARCH завершился {status!r}")
    identifiers = next(iter(matches), b"").split()
    match identifiers:
        case [identifier]:
            pass
        case _:
            raise AssertionError(f"ожидалось одно письмо, получено {identifiers!r}")
    status, records = imap.fetch(identifier, "(BODYSTRUCTURE RFC822.HEADER)")
    if ImapStatus.parse(status) is not ImapStatus.OK:
        raise AssertionError(f"IMAP FETCH завершился {status!r}: {records!r}")
    fragments: list[bytes] = []
    for record in records:
        match record:
            case (metadata, payload) if isinstance(metadata, bytes) and isinstance(payload, bytes):
                fragments.extend((metadata, payload))
            case bytes() as line:
                fragments.append(line)
            case _:
                raise AssertionError(f"неожиданный FETCH record: {record!r}")
    return b" ".join(fragments)


def run(options: argparse.Namespace) -> None:
    """Передаёт письмо Alice и проверяет RFC 2231 представление у Bob."""
    marker = options.marker
    subject = f"RFC2231 filename {marker}"
    raw_message = message_bytes(marker)
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    with smtplib.SMTP(options.host, options.smtp_port, timeout=15) as smtp:
        smtp.starttls(context=context)
        smtp.login("alice", password_for(options.credentials, "alice"))
        refused = smtp.sendmail(
            "alice@litterkeep.test", ["bob@litterkeep.test"], raw_message
        )
        if refused:
            raise AssertionError(f"SMTP отклонил получателей: {refused!r}")
    with imaplib.IMAP4(options.host, options.imap_port, timeout=15) as imap:
        imap.starttls(ssl_context=context)
        status, _ = imap.login("bob", password_for(options.credentials, "bob"))
        if ImapStatus.parse(status) is not ImapStatus.OK:
            raise AssertionError(f"IMAP LOGIN завершился {status!r}")
        status, _ = imap.select("INBOX", readonly=True)
        if ImapStatus.parse(status) is not ImapStatus.OK:
            raise AssertionError(f"IMAP SELECT завершился {status!r}")
        fetched = fetch_message(imap, subject)
    if b"filename*0*=UTF-8'en-US'%F0%9F%98%80%20" not in fetched:
        raise AssertionError("исходный language-tag/первый RFC 2231 сегмент изменён")
    if b"filename*1*=r%C3%A9sum%C3%A9.txt" not in fetched:
        raise AssertionError("исходный второй RFC 2231 сегмент изменён")
    if "😀 résumé.txt".encode("utf-8") not in fetched:
        raise AssertionError("BODYSTRUCTURE не объединил и не декодировал filename")
    print("PASS SMTP-to-IMAP RFC 2231 language-tagged filename continuation")


PARSER = argparse.ArgumentParser()
PARSER.add_argument("--host", default="127.0.0.1")
PARSER.add_argument("--smtp-port", type=int, default=62587)
PARSER.add_argument("--imap-port", type=int, default=62143)
PARSER.add_argument("--credentials", type=Path, required=True)
PARSER.add_argument("--marker", required=True)
run(PARSER.parse_args())
