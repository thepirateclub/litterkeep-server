"""Проверяет MIME single-byte charset декодирование через SMTP и IMAP SEARCH."""

from __future__ import annotations

import argparse
import imaplib
import smtplib
import socket
import ssl
import time
from dataclasses import dataclass
from enum import Enum
from pathlib import Path


class ClientStatus(Enum):
    """Закрытый набор итоговых состояний независимых protocol clients."""

    OK = "OK"
    FAILED = "FAILED"

    @classmethod
    def parse(cls, value: str) -> ClientStatus:
        """Преобразует ответ protocol boundary в типизированное состояние."""
        match value:
            case "OK":
                return cls.OK
            case _:
                return cls.FAILED


@dataclass(frozen=True)
class CharsetCase:
    """Описывает MIME label, независимый codec и тестовый Unicode текст."""

    label: str
    codec: str
    phrase: str


CHARSET_CASES = (
    CharsetCase("iso-8859-1", "iso8859_1", "café"),
    CharsetCase("iso-8859-2", "iso8859_2", "Zażółć"),
    CharsetCase("iso-8859-3", "iso8859_3", "AfrĦika"),
    CharsetCase("iso-8859-4", "iso8859_4", "RigaĄ"),
    CharsetCase("iso-8859-5", "iso8859_5", "Привет"),
    CharsetCase("iso-8859-6", "iso8859_6", "الاسلام"),
    CharsetCase("iso-8859-7", "iso8859_7", "Ελλάδα"),
    CharsetCase("iso-8859-8", "iso8859_8", "שלום"),
    CharsetCase("iso-8859-9", "iso8859_9", "İstanbul"),
    CharsetCase("iso-8859-10", "iso8859_10", "Ísland"),
    CharsetCase("iso-8859-13", "iso8859_13", "Šiauliai"),
    CharsetCase("iso-8859-14", "iso8859_14", "ḂĠ Cymru"),
    CharsetCase("iso-8859-15", "iso8859_15", "Euro €"),
    CharsetCase("iso-8859-16", "iso8859_16", "ȘȚ șț"),
    CharsetCase("TIS-620", "tis_620", "ภาษาไทย"),
    CharsetCase("ISO-8859-11", "iso8859_11", "ภาษาไทย"),
    CharsetCase("IBM866", "cp866", "Привет"),
    CharsetCase("KOI8-R", "koi8_r", "Привет"),
    CharsetCase("KOI8-U", "koi8_u", "Їжак"),
    CharsetCase("windows-874", "cp874", "ภาษาไทย"),
    CharsetCase("windows-1250", "cp1250", "Zażółć"),
    CharsetCase("windows-1251", "cp1251", "Привет"),
    CharsetCase("windows-1252", "cp1252", "café"),
    CharsetCase("windows-1253", "cp1253", "Ελλάδα"),
    CharsetCase("windows-1254", "cp1254", "İstanbul"),
    CharsetCase("windows-1255", "cp1255", "שלום"),
    CharsetCase("windows-1256", "cp1256", "الاسلام"),
    CharsetCase("windows-1257", "cp1257", "Šiauliai"),
    CharsetCase("windows-1258", "cp1258", "ma\u0300u"),
)


def password_for(credentials: Path, username: str) -> str:
    """Извлекает пароль заданного аккаунта из локального acceptance fixture."""
    rows = (line.split("\t", maxsplit=1) for line in credentials.read_text(encoding="utf-8").splitlines())
    for row in rows:
        match row:
            case [name, password] if name == username:
                return password
            case _:
                continue
    raise AssertionError(f"аккаунт {username!r} отсутствует в credentials fixture")


def wait_for_listener(host: str, port: int) -> None:
    """Ожидает SMTP/IMAP listener с конечным таймаутом."""
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        try:
            with socket.create_connection((host, port), timeout=0.5):
                return
        except OSError:
            time.sleep(0.1)
    raise TimeoutError(f"listener {host}:{port} не открылся за 15 секунд")


def build_message() -> tuple[bytes, dict[bytes, bytes]]:
    """Формирует multipart сообщение и независимые UTF-8 поисковые ключи."""
    boundary = b"litterkeep-singlebyte-boundary"
    parts = []
    queries = {}
    for ordinal, case in enumerate(CHARSET_CASES, start=1):
        phrase = f"{case.phrase} matrix{ordinal:02d}"
        raw_body = phrase.encode(case.codec, errors="strict")
        query = phrase.encode("utf-8")
        queries[query] = case.label.encode("ascii")
        parts.extend(
            (
                b"--" + boundary,
                f"Content-Type: text/plain; charset={case.label}".encode("ascii"),
                b"Content-Transfer-Encoding: 8bit",
                b"",
                raw_body,
            )
        )
    parts.append(b"--" + boundary + b"--")
    body = b"\r\n".join(parts)
    message = (
        b"From: Alice <alice@litterkeep.test>\r\n"
        b"To: Bob <bob@litterkeep.test>\r\n"
        b"Subject: single-byte MIME charset matrix\r\n"
        b"MIME-Version: 1.0\r\n"
        b"Content-Type: multipart/mixed; boundary=litterkeep-singlebyte-boundary\r\n"
        b"\r\n"
        + body
        + b"\r\n"
    )
    return message, queries


def deliver_message(options: argparse.Namespace, context: ssl.SSLContext, message: bytes) -> None:
    """Передаёт подготовленную multipart матрицу Alice через SMTP STARTTLS."""
    with smtplib.SMTP(options.host, options.smtp_port, timeout=15) as smtp:
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


def search_literal(imap: imaplib.IMAP4, query: bytes) -> bytes:
    """Ищет один UTF-8 ключ synchronizing literal и возвращает ответ сервера."""
    tag = b"z321"
    imap.send(tag + b" SEARCH CHARSET UTF-8 BODY {" + str(len(query)).encode() + b"}\r\n")
    continuation = imap.readline()
    if not continuation.startswith(b"+"):
        raise AssertionError(f"сервер не запросил SEARCH literal: {continuation!r}")
    imap.send(query + b"\r\n")
    responses = []
    while True:
        response = imap.readline()
        responses.append(response)
        if response.startswith(tag + b" "):
            break
    joined = b" ".join(responses)
    match responses[-1].split():
        case [response_tag, b"OK", *_] if response_tag == tag:
            status = ClientStatus.OK
        case _:
            status = ClientStatus.FAILED
    if status is not ClientStatus.OK:
        raise AssertionError(f"SEARCH завершился ошибкой: {joined!r}")
    if b"* SEARCH " not in joined:
        raise AssertionError(f"SEARCH не вернул совпадение для {query!r}: {joined!r}")
    return joined


def verify_queries(options: argparse.Namespace, context: ssl.SSLContext, queries: dict[bytes, bytes]) -> None:
    """Проверяет независимым IMAP клиентом все decoded MIME charset parts."""
    with imaplib.IMAP4(options.host, options.imap_port, timeout=15) as imap:
        imap.starttls(ssl_context=context)
        login_status, _ = imap.login("bob", password_for(options.credentials, "bob"))
        if ClientStatus.parse(login_status) is not ClientStatus.OK:
            raise AssertionError(f"IMAP LOGIN завершился статусом {login_status!r}")
        select_status, _ = imap.select("INBOX", readonly=True)
        if ClientStatus.parse(select_status) is not ClientStatus.OK:
            raise AssertionError(f"IMAP SELECT завершился статусом {select_status!r}")
        for query, label in queries.items():
            search_literal(imap, query)
            print(f"PASS IMAP SEARCH decoded MIME charset={label.decode('ascii')}")


def run(options: argparse.Namespace) -> None:
    """Проверяет цепочку SMTP delivery -> MIME charset -> IMAP SEARCH."""
    wait_for_listener(options.host, options.smtp_port)
    wait_for_listener(options.host, options.imap_port)
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    message, queries = build_message()
    deliver_message(options, context, message)
    verify_queries(options, context, queries)
    print(f"PASS single-byte MIME charset matrix: {len(queries)} labels")


PARSER = argparse.ArgumentParser()
PARSER.add_argument("--host", default="127.0.0.1")
PARSER.add_argument("--smtp-port", type=int, default=62589)
PARSER.add_argument("--imap-port", type=int, default=62145)
PARSER.add_argument("--credentials", type=Path, required=True)
run(PARSER.parse_args())
