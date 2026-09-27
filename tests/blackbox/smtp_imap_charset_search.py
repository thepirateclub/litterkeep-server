"""Проверяет декодирование MIME charset в независимом IMAP SEARCH запросе."""

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
    """Закрытый набор успешных и неуспешных ответов protocol clients."""

    OK = "OK"
    FAILED = "FAILED"

    @classmethod
    def parse(cls, value: str) -> ClientStatus:
        """Нормализует строковый ответ без дальнейших сравнений по тексту."""
        match value:
            case "OK":
                return cls.OK
            case _:
                return cls.FAILED


def password_for(credentials: Path, username: str) -> str:
    """Читает пароль выбранной acceptance учётной записи из локального Store."""
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
    """Ожидает готовность указанного listener-а с ограниченным сроком."""
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        try:
            with socket.create_connection((host, port), timeout=0.5):
                return
        except OSError:
            time.sleep(0.1)
    raise TimeoutError(f"listener {host}:{port} не открылся за 15 секунд")


def send_legacy_charset_message(options: argparse.Namespace, context: ssl.SSLContext) -> None:
    """Доставляет тайское MIME body с зарегистрированным Windows-874 charset."""
    body = bytes.fromhex("c0d2c9d2e4b7c2")
    message = (
        b"From: Alice <alice@litterkeep.test>\r\n"
        b"To: Bob <bob@litterkeep.test>\r\n"
        b"Subject: Windows-874 IMAP search fixture\r\n"
        b"MIME-Version: 1.0\r\n"
        b"Content-Type: text/plain; charset=windows-874\r\n"
        b"Content-Transfer-Encoding: 8bit\r\n"
        b"\r\n"
        + body
        + b"\r\n"
    )
    with smtplib.SMTP(options.host, options.smtp_port, timeout=10) as smtp:
        smtp.starttls(context=context)
        smtp.login("alice", password_for(options.credentials, "alice"))
        refused = smtp.sendmail(
            "alice@litterkeep.test",
            ["bob@litterkeep.test"],
            message,
            mail_options=["BODY=8BITMIME"],
        )
        if refused:
            raise AssertionError(f"SMTP отклонил адресатов: {refused!r}")


def search_for_decoded_text(options: argparse.Namespace, context: ssl.SSLContext) -> None:
    """Ищет Unicode text через synchronizing literal после TLS и LOGIN."""
    expected = "ภาษาไทย".encode("utf-8")
    tag = b"z321"
    with imaplib.IMAP4(options.host, options.imap_port, timeout=10) as imap:
        imap.starttls(ssl_context=context)
        login_status, _ = imap.login("bob", password_for(options.credentials, "bob"))
        if ClientStatus.parse(login_status) is not ClientStatus.OK:
            raise AssertionError(f"IMAP LOGIN завершился статусом {login_status!r}")
        select_status, _ = imap.select("INBOX", readonly=True)
        if ClientStatus.parse(select_status) is not ClientStatus.OK:
            raise AssertionError(f"IMAP SELECT завершился статусом {select_status!r}")

        command = tag + b" SEARCH CHARSET UTF-8 BODY {" + str(len(expected)).encode() + b"}\r\n"
        imap.send(command)
        continuation = imap.readline()
        if not continuation.startswith(b"+"):
            raise AssertionError(f"сервер не запросил SEARCH literal: {continuation!r}")
        imap.send(expected + b"\r\n")
        responses = []
        completion = b""
        while True:
            response = imap.readline()
            responses.append(response)
            if response.startswith(tag + b" "):
                completion = response
                break
        joined = b" ".join(responses)
        match completion.split():
            case [response_tag, b"OK", *_] if response_tag == tag:
                status = ClientStatus.OK
            case _:
                status = ClientStatus.FAILED
        if status is not ClientStatus.OK:
            raise AssertionError(f"SEARCH завершился ошибкой: {joined!r}")
        if b"* SEARCH " not in joined:
            raise AssertionError(f"SEARCH не вернул совпавшее сообщение: {joined!r}")


def run(options: argparse.Namespace) -> None:
    """Проверяет цепочку SMTP ingest, MIME decoding и IMAP SEARCH."""
    wait_for_listener(options.host, options.smtp_port)
    wait_for_listener(options.host, options.imap_port)
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    send_legacy_charset_message(options, context)
    search_for_decoded_text(options, context)
    print("PASS IMAP SEARCH decoded Windows-874 MIME text")


PARSER = argparse.ArgumentParser()
PARSER.add_argument("--host", default="127.0.0.1")
PARSER.add_argument("--smtp-port", type=int, default=62588)
PARSER.add_argument("--imap-port", type=int, default=62144)
PARSER.add_argument("--credentials", type=Path, required=True)
run(PARSER.parse_args())
