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


ATTACHMENT = bytes(range(32)) + b"\x00litterkeep-attachment\xff"


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


def fetched_message(host: str, port: int, context: ssl.SSLContext) -> bytes:
    """Читает первое письмо Bob через отдельную TLS IMAP-сессию."""
    with imaplib.IMAP4_SSL(host, port, ssl_context=context, timeout=10) as client:
        client.login("bob", "bob-password")
        client.select("INBOX", readonly=True)
        status, response = client.fetch("1", "(RFC822)")
        if status != "OK":
            raise AssertionError(f"IMAP FETCH failed: {status} {response!r}")
        for item in response:
            match item:
                case (_, bytes() as payload):
                    return payload
                case _:
                    continue
    raise AssertionError("IMAP FETCH returned no RFC822 literal")


def attachment_bytes(message: bytes) -> bytes:
    """Находит вложение по MIME filename без позиционного доступа к массиву."""
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
    actual = attachment_bytes(
        fetched_message(options.host, options.imap_port, context)
    )
    if actual != ATTACHMENT:
        raise AssertionError("attachment bytes changed during mail path")
    digest = hashlib.sha256(actual).hexdigest()
    print(f"TLS SMTP AUTH to IMAP attachment passed: sha256={digest}")


if __name__ == "__main__":
    main()
