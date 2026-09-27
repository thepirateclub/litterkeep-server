"""Проверяет wire SEARCH по RFC 2047 UTF-16/UTF-32 encoded-word."""

from __future__ import annotations

import argparse
import base64
import imaplib
import smtplib
import ssl
import uuid
from enum import Enum
from pathlib import Path


class EncodedWordCharset(Enum):
    """Закрытый набор проверяемых IANA endian-вариантов."""

    UTF16 = ("UTF-16", "utf-16")
    UTF16_BE = ("UTF-16BE", "utf-16-be")
    UTF16_LE = ("UTF-16LE", "utf-16-le")
    UTF32 = ("UTF-32", "utf-32-be")
    UTF32_BE = ("UTF-32BE", "utf-32-be")
    UTF32_LE = ("UTF-32LE", "utf-32-le")

    @property
    def label(self) -> str:
        """Возвращает wire charset label, объявленный в encoded-word."""
        match self.value:
            case (label, _codec):
                return label
            case _:
                raise AssertionError("invalid charset enum payload")

    @property
    def codec(self) -> str:
        """Возвращает независимый Python strict encoder для wire-вектора."""
        match self.value:
            case (_label, codec):
                return codec
            case _:
                raise AssertionError("invalid charset enum payload")


class ImapStatus(Enum):
    """Типизированные результаты команд, используемые acceptance-клиентом."""

    OK = "OK"
    ERROR = "ERROR"

    @classmethod
    def parse(cls, value: str | bytes) -> ImapStatus:
        """Сопоставляет wire status закрытому enum."""
        match value:
            case "OK" | b"OK":
                return cls.OK
            case _:
                return cls.ERROR


def encoded_octets(charset: EncodedWordCharset, text: str) -> bytes:
    """Строит RFC 2047 Base64 encoded-word для указанной endian-схемы."""
    match charset:
        case EncodedWordCharset.UTF32:
            octets = b"\x00\x00\xfe\xff" + text.encode(charset.codec, errors="strict")
        case _:
            octets = text.encode(charset.codec, errors="strict")
    payload = base64.b64encode(octets)
    return b"=?" + charset.label.encode("ascii") + b"?B?" + payload + b"?="


def message(subject: bytes) -> bytes:
    """Собирает независимое ASCII wire-письмо с единственным encoded-word."""
    return b"\r\n".join(
        (
            b"From: alice@litterkeep.test",
            b"To: bob@litterkeep.test",
            b"Subject: " + subject,
            b"MIME-Version: 1.0",
            b"Content-Type: text/plain; charset=us-ascii",
            b"",
            b"body",
            b"",
        )
    )


def password_for(credentials: Path, username: str) -> str:
    """Читает тестовый пароль из локального файла, не выводя его в argv."""
    for line in credentials.read_text(encoding="utf-8").splitlines():
        account, separator, password = line.partition("\t")
        match (account, separator):
            case (found, "\t") if found == username:
                return password
            case _:
                continue
    raise ValueError(f"аккаунт {username!r} отсутствует в fixture")


def deliver_vectors(
    options: argparse.Namespace, context: ssl.SSLContext
) -> dict[EncodedWordCharset, str]:
    """Доставляет по SMTP письмо для каждого generic и explicit charset."""
    nonce = uuid.uuid4().hex
    searches: dict[EncodedWordCharset, str] = {}
    with smtplib.SMTP(options.host, options.smtp_port, timeout=10) as smtp:
        smtp.starttls(context=context)
        smtp.login("alice", password_for(options.credentials, "alice"))
        for charset in EncodedWordCharset:
            prefix = f"wire-{nonce}-{charset.label}"
            expected = f"{prefix} café 東京"
            subject = prefix.encode("ascii") + b" " + encoded_octets(
                charset, "café 東京"
            )
            refused = smtp.sendmail(
                "alice@litterkeep.test",
                ["bob@litterkeep.test"],
                message(subject),
            )
            if refused:
                raise AssertionError(f"SMTP отклонил {charset.label}: {refused!r}")
            searches[charset] = expected
    return searches


def search_literal(imap: imaplib.IMAP4, needle: bytes, tag: bytes) -> bytes:
    """Передаёт UTF-8 SEARCH literal и возвращает завершённый wire frame."""
    command = tag + b" SEARCH CHARSET UTF-8 SUBJECT {" + str(len(needle)).encode() + b"}\r\n"
    imap.send(command)
    continuation = imap.readline()
    if not continuation.startswith(b"+"):
        raise AssertionError(f"SEARCH literal continuation отсутствует: {continuation!r}")
    imap.send(needle + b"\r\n")
    responses: list[bytes] = []
    while True:
        response = imap.readline()
        responses.append(response)
        if response.startswith(tag + b" "):
            break
    frame = b"".join(responses)
    search_rows = [line for line in responses if line.startswith(b"* SEARCH ")]
    match search_rows:
        case [search_row]:
            match search_row.split():
                case [b"*", b"SEARCH", message_number] if message_number.isdigit():
                    pass
                case _:
                    raise AssertionError(f"SEARCH не вернул один UID: {frame!r}")
        case _:
            raise AssertionError(f"SEARCH не вернул единственный результат: {frame!r}")
    completion = next(
        (line for line in responses if line.startswith(tag + b" ")),
        b"",
    )
    match completion.split():
        case [response_tag, status, b"SEARCH", b"completed"] if response_tag == tag:
            if ImapStatus.parse(status) is not ImapStatus.OK:
                raise AssertionError(f"SEARCH завершился отказом: {frame!r}")
        case _:
            raise AssertionError(f"SEARCH завершился неожиданно: {frame!r}")
    return frame


def verify(options: argparse.Namespace) -> None:
    """Сверяет декодирование Subject на SMTP и IMAP сетевой границе."""
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    expected = deliver_vectors(options, context)
    with imaplib.IMAP4(options.host, options.imap_port, timeout=10) as imap:
        status, _ = imap.starttls(ssl_context=context)
        if ImapStatus.parse(status) is not ImapStatus.OK:
            raise AssertionError(f"IMAP STARTTLS: {status}")
        status, _ = imap.login("bob", password_for(options.credentials, "bob"))
        if ImapStatus.parse(status) is not ImapStatus.OK:
            raise AssertionError(f"IMAP LOGIN: {status}")
        status, _ = imap.select("INBOX", readonly=True)
        if ImapStatus.parse(status) is not ImapStatus.OK:
            raise AssertionError(f"IMAP SELECT: {status}")
        for ordinal, (charset, text) in enumerate(expected.items(), start=1):
            tag = f"z{ordinal:03}".encode("ascii")
            search_literal(imap, text.encode("utf-8"), tag)


def main() -> None:
    """Запускает независимый SMTP -> MIME -> IMAP SEARCH acceptance."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, default=62587)
    parser.add_argument("--imap-port", type=int, default=62143)
    parser.add_argument("--credentials", type=Path, required=True)
    verify(parser.parse_args())
    print("PASS IMAP SEARCH RFC 2047 UTF-16/UTF-32: 6 charset labels")


if __name__ == "__main__":
    main()
