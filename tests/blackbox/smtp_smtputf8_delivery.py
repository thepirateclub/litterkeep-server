"""Проверяет Unicode SMTP envelope и байты MIME-вложения через IMAP."""

from __future__ import annotations

import argparse
import base64
import contextlib
import hashlib
import imaplib
import smtplib
import ssl
from email import policy
from email.parser import BytesParser
from enum import Enum, IntEnum
from pathlib import Path


class SmtpReply(IntEnum):
    """Итоговые SMTP-коды проверяемой доставки."""

    ACCEPTED = 250
    DATA_READY = 354


class ImapStatus(Enum):
    """Состояния IMAP-операций после преобразования библиотечного ответа."""

    OK = "OK"
    FAILED = "FAILED"

    @classmethod
    def parse(cls, value: str) -> ImapStatus:
        """Сводит библиотечный статус к закрытому набору исходов."""
        match value:
            case "OK":
                return cls.OK
            case _:
                return cls.FAILED


def password_for(credentials: Path, username: str) -> str:
    """Читает только пароль указанного acceptance аккаунта."""
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


def message_bytes(attachment: bytes) -> bytes:
    """Создаёт MIME multipart сообщение с ASCII headers и base64 binary part."""
    encoded = base64.encodebytes(attachment).replace(b"\n", b"\r\n")
    return (
        b"From: Alice <alice@litterkeep.test>\r\n"
        b"To: SMTPUTF8 delivery probe\r\n"
        b"Subject: SMTPUTF8 mailbox attachment\r\n"
        b"MIME-Version: 1.0\r\n"
        b"Content-Type: multipart/mixed; boundary=lk-eai-boundary\r\n\r\n"
        b"--lk-eai-boundary\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n"
        b"UTF-8 envelope recipient acceptance\r\n"
        b"--lk-eai-boundary\r\n"
        b"Content-Type: application/octet-stream; name=fixture.bin\r\n"
        b"Content-Transfer-Encoding: base64\r\n"
        b"Content-Disposition: attachment; filename=fixture.bin\r\n\r\n"
        + encoded
        + b"--lk-eai-boundary--\r\n"
    )


def require_smtp_reply(code: int, expected: SmtpReply, response: bytes) -> None:
    """Сравнивает SMTP-код с типизированным ожиданием acceptance."""
    if code != expected:
        raise AssertionError(f"ожидался SMTP {expected.value}, получен {code}: {response!r}")


def submit_message(
    host: str,
    port: int,
    context: ssl.SSLContext,
    alice_password: str,
    message: bytes,
) -> None:
    """Проверяет скрытый SMTPUTF8 путь, не выдавая peer за совместимый клиент."""
    with smtplib.SMTP(host, port, timeout=10) as client:
        client.ehlo()
        client.starttls(context=context)
        client.ehlo()
        if "smtputf8" in client.esmtp_features:
            raise AssertionError("сервер не должен рекламировать неполную SMTPUTF8")
        client.login("alice", alice_password)

        # EHLO намеренно не предлагает SMTPUTF8: это принудительный probe
        # внутреннего пути сервера, а не поведение соответствующего RFC клиента.
        client.send(b"MAIL FROM:<alice@litterkeep.test> SMTPUTF8\r\n")
        code, response = client.getreply()
        require_smtp_reply(code, SmtpReply.ACCEPTED, response)

        recipient = "RCPT TO:<пользователь@litterkeep.test>\r\n".encode("utf-8")
        client.send(recipient)
        code, response = client.getreply()
        require_smtp_reply(code, SmtpReply.ACCEPTED, response)

        code, response = client.data(message)
        require_smtp_reply(code, SmtpReply.ACCEPTED, response)


def read_attachment(
    host: str,
    port: int,
    context: ssl.SSLContext,
    password: str,
) -> bytes:
    """Входит UTF-8 authcid через SASL PLAIN и читает доставленный RFC822."""
    mailbox = imaplib.IMAP4(host, port, timeout=10)
    mailbox.starttls(ssl_context=context)
    authcid = "пользователь".encode("utf-8")
    secret = password.encode("utf-8")
    status, detail = mailbox.authenticate(
        "PLAIN",
        lambda _challenge: b"\0" + authcid + b"\0" + secret,
    )
    if ImapStatus.parse(status) is not ImapStatus.OK:
        raise AssertionError(f"IMAP SASL PLAIN не прошёл: {detail!r}")
    try:
        status, detail = mailbox.select("INBOX", readonly=True)
        if ImapStatus.parse(status) is not ImapStatus.OK:
            raise AssertionError(f"INBOX не выбран: {detail!r}")
        status, records = mailbox.fetch("1", "(RFC822)")
        if ImapStatus.parse(status) is not ImapStatus.OK:
            raise AssertionError(f"RFC822 FETCH не выполнен: {records!r}")
        raw = fetched_message(records)
        parsed = BytesParser(policy=policy.default).parsebytes(raw)
        return next(
            part.get_payload(decode=True)
            for part in parsed.iter_attachments()
            if part.get_filename() == "fixture.bin"
        )
    finally:
        with contextlib.suppress(OSError, imaplib.IMAP4.error):
            mailbox.logout()


def fetched_message(records: list[object]) -> bytes:
    """Извлекает literal FETCH без позиционного доступа к кортежу ответа."""
    for record in records:
        match record:
            case (_metadata, payload) if isinstance(payload, bytes):
                return payload
            case _:
                continue
    raise AssertionError(f"FETCH response не содержит RFC822 literal: {records!r}")


def main() -> None:
    """Запускает SMTP-to-IMAP octet comparison для одного чистого fixture."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--smtp-port", type=int, required=True)
    parser.add_argument("--imap-port", type=int, required=True)
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    parser.add_argument("--unicode-credentials", type=Path, required=True)
    parser.add_argument("--read-only", action="store_true")
    options = parser.parse_args()
    context = ssl.create_default_context(cafile=str(options.ca))
    alice_password = password_for(options.credentials, "alice")
    recipient_password = password_for(options.unicode_credentials, "пользователь")
    attachment = bytes(range(256)) * 2 + b"\x00litterkeep-eai\xff"
    match options.read_only:
        case False:
            submit_message(
                "127.0.0.1",
                options.smtp_port,
                context,
                alice_password,
                message_bytes(attachment),
            )
        case True:
            pass
    actual = read_attachment("127.0.0.1", options.imap_port, context, recipient_password)
    expected_digest = hashlib.sha256(attachment).hexdigest()
    actual_digest = hashlib.sha256(actual).hexdigest()
    if actual != attachment:
        raise AssertionError(f"attachment octets changed: {expected_digest} != {actual_digest}")
    print(f"PASS smtp-smtputf8-delivery: attachment sha256={actual_digest}")


if __name__ == "__main__":
    main()
