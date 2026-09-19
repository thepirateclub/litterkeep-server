"""Сквозная независимая проверка TLS submission и TLS IMAP чтения.

Используются только клиенты стандартной библиотеки Python. Тест не вызывает
внутренние Rust API и сравнивает байты MIME-вложения после полного wire-пути.
"""

import argparse
import email.policy
import hashlib
import imaplib
import smtplib
import ssl
from email.message import EmailMessage
from email.parser import BytesParser
from enum import Enum


ATTACHMENT = bytes(range(32)) + b"\x00litterkeep-attachment\xff"


class ImapStatus(Enum):
    """Типизированный статус ответа стандартного IMAP-клиента."""

    OK = "OK"


def arguments() -> argparse.Namespace:
    """Разбирает адреса listener-ов и доверенный тестовый CA."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("smtp_port", type=int)
    parser.add_argument("imap_port", type=int)
    parser.add_argument("ca_file")
    return parser.parse_args()


def message_bytes() -> bytes:
    """Создаёт письмо с бинарным вложением и стабильными заголовками."""
    message = EmailMessage()
    message["From"] = "Alice <alice@test>"
    message["To"] = "Bob <bob@test>"
    message["Subject"] = "TLS SMTP AUTH attachment"
    message.set_content("Authenticated delivery acceptance.\n")
    message.add_attachment(
        ATTACHMENT,
        maintype="application",
        subtype="octet-stream",
        filename="evidence.bin",
    )
    return message.as_bytes(policy=email.policy.SMTP)


def submit(host: str, port: int, context: ssl.SSLContext, payload: bytes) -> None:
    """Аутентифицирует Alice стандартным SMTP_SSL и отправляет Bob."""
    with smtplib.SMTP_SSL(host, port, context=context, timeout=10) as client:
        client.login("alice", "alice-password")
        client.sendmail("alice@test", ("bob@test",), payload)


def fetched_messages(host: str, port: int, context: ssl.SSLContext) -> list[bytes]:
    """Читает снимок INBOX Bob через отдельную TLS IMAP-сессию."""
    messages: list[bytes] = []
    with imaplib.IMAP4_SSL(host, port, ssl_context=context, timeout=10) as client:
        client.login("bob", "bob-password")
        _, counts = client.select("INBOX", readonly=True)
        count = int(next(iter(counts)))
        for number in range(1, count + 1):
            status, response = client.fetch(str(number), "(RFC822)")
            match ImapStatus(status):
                case ImapStatus.OK:
                    pass
            for item in response:
                match item:
                    case (_, bytes() as payload):
                        messages.append(payload)
                    case _:
                        continue
    return messages


def attachment_bytes(messages: list[bytes]) -> bytes:
    """Находит вложение во всём snapshot без позиционного доступа к массиву."""
    for message in messages:
        parsed = BytesParser(policy=email.policy.default).parsebytes(message)
        for part in parsed.iter_attachments():
            if part.get_filename() == "evidence.bin":
                payload = part.get_payload(decode=True)
                if isinstance(payload, bytes):
                    return payload
    raise AssertionError("evidence.bin is absent from delivered MIME message")


def main() -> None:
    """Выполняет submission, delivery, retrieval и байтовое сравнение."""
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca_file)
    payload = message_bytes()
    submit(options.host, options.smtp_port, context, payload)
    actual = attachment_bytes(fetched_messages(options.host, options.imap_port, context))
    if actual != ATTACHMENT:
        raise AssertionError("attachment bytes changed during mail path")
    digest = hashlib.sha256(actual).hexdigest()
    print(f"TLS SMTP AUTH to IMAP attachment passed: sha256={digest}")


if __name__ == "__main__":
    main()
