"""Проверяет UTF-32 MIME формы через SMTP delivery и IMAP BODYSTRUCTURE."""

from __future__ import annotations

import argparse
import imaplib
import smtplib
import socket
import ssl
import time
from enum import Enum
from pathlib import Path


class ClientStatus(Enum):
    """Нормализованный итог операций стандартных SMTP/IMAP клиентов."""

    OK = "OK"
    FAILED = "FAILED"

    @classmethod
    def parse(cls, value: str) -> ClientStatus:
        """Преобразует ответ библиотеки в закрытый набор состояний."""
        match value:
            case "OK":
                return cls.OK
            case _:
                return cls.FAILED


def password_for(credentials: Path, username: str) -> str:
    """Читает пароль только выбранного локального acceptance аккаунта."""
    entries = (
        line.split("\t", maxsplit=1)
        for line in credentials.read_text(encoding="utf-8").splitlines()
    )
    for entry in entries:
        match entry:
            case [name, password] if name == username:
                return password
            case _:
                continue
    raise AssertionError(f"аккаунт {username!r} отсутствует в credentials fixture")


def wait_for_listener(host: str, port: int) -> None:
    """Ожидает публикацию порта и выдаёт диагностическую ошибку по сроку."""
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        try:
            with socket.create_connection((host, port), timeout=0.5):
                return
        except OSError:
            time.sleep(0.1)
    raise TimeoutError(f"listener {host}:{port} не открылся за 15 секунд")


def message_bytes() -> bytes:
    """Формирует сообщение с UTF-32BE RFC 2047 и RFC 2231 значениями."""
    return (
        b"From: Alice <alice@litterkeep.test>\r\n"
        b"To: Bob <bob@litterkeep.test>\r\n"
        b"Subject: =?UTF-32BE?B?AAH2AA==?=\r\n"
        b"MIME-Version: 1.0\r\n"
        b"Content-Type: application/octet-stream\r\n"
        b"Content-Transfer-Encoding: base64\r\n"
        b"Content-Disposition: attachment; filename*=UTF-32BE''%00%01%F6%00\r\n"
        b"\r\nWA==\r\n"
    )


def fetch_wire(imap: imaplib.IMAP4, sequence: int) -> bytes:
    """Собирает metadata и RFC822 header literal из стандартного FETCH ответа."""
    status, records = imap.fetch(str(sequence), "(BODYSTRUCTURE RFC822.HEADER)")
    if ClientStatus.parse(status) is not ClientStatus.OK:
        raise AssertionError(f"FETCH завершился статусом {status!r}: {records!r}")
    fragments: list[bytes] = []
    for record in records:
        match record:
            case (metadata, payload) if isinstance(metadata, bytes) and isinstance(payload, bytes):
                fragments.extend((metadata, payload))
            case bytes() as line:
                fragments.append(line)
            case _:
                raise AssertionError(f"неожиданная форма FETCH ответа: {record!r}")
    return b" ".join(fragments)


def run(options: argparse.Namespace) -> None:
    """Отправляет письмо Alice и проверяет его представление у Bob."""
    wait_for_listener(options.host, options.smtp_port)
    wait_for_listener(options.host, options.imap_port)
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    message = message_bytes()

    with smtplib.SMTP(options.host, options.smtp_port, timeout=10) as smtp:
        smtp.starttls(context=context)
        smtp.login("alice", password_for(options.credentials, "alice"))
        refused = smtp.sendmail(
            "alice@litterkeep.test", ["bob@litterkeep.test"], message
        )
        if refused:
            raise AssertionError(f"SMTP отклонил адресатов: {refused!r}")

    with imaplib.IMAP4(options.host, options.imap_port, timeout=10) as imap:
        imap.starttls(ssl_context=context)
        login_status, _ = imap.login("bob", password_for(options.credentials, "bob"))
        if ClientStatus.parse(login_status) is not ClientStatus.OK:
            raise AssertionError(f"IMAP LOGIN завершился статусом {login_status!r}")
        select_status, count = imap.select("INBOX", readonly=True)
        if ClientStatus.parse(select_status) is not ClientStatus.OK:
            raise AssertionError(f"IMAP SELECT завершился статусом {select_status!r}")
        sequence = int(next(iter(count)))
        wire = fetch_wire(imap, sequence)

    encoded_subject = b"Subject: =?UTF-32BE?B?AAH2AA==?="
    decoded_filename = "😀".encode("utf-8")
    if encoded_subject not in wire:
        raise AssertionError("FETCH не сохранил исходный UTF-32BE encoded-word")
    if decoded_filename not in wire:
        raise AssertionError("BODYSTRUCTURE не декодировал RFC 2231 UTF-32BE filename")
    print("PASS SMTP-to-IMAP UTF-32BE encoded-word and attachment filename")


PARSER = argparse.ArgumentParser()
PARSER.add_argument("--host", default="127.0.0.1")
PARSER.add_argument("--smtp-port", type=int, default=62587)
PARSER.add_argument("--imap-port", type=int, default=62143)
PARSER.add_argument("--credentials", type=Path, required=True)
run(PARSER.parse_args())
