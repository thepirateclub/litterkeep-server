"""Независимая проверка одной TLS submission для двух local recipients."""

import argparse
import email.policy
import imaplib
import smtplib
import ssl
from email.message import EmailMessage
from email.parser import BytesParser
from enum import Enum


SUBJECT = "atomic multi recipient acceptance"
ATTACHMENT = b"one-blob-two-mailbox-links\x00\xff"


class ImapStatus(Enum):
    """Типизированный статус стандартного IMAP-клиента."""

    OK = "OK"


class Action(Enum):
    """Фаза, разделяемая внешним process kill/restart."""

    SUBMIT = "submit"
    VERIFY = "verify"


def arguments() -> argparse.Namespace:
    """Читает TLS endpoints и CA."""
    parser = argparse.ArgumentParser()
    parser.add_argument("action", type=Action)
    parser.add_argument("host")
    parser.add_argument("smtp_port", type=int)
    parser.add_argument("imap_port", type=int)
    parser.add_argument("ca_file")
    return parser.parse_args()


def message_bytes() -> bytes:
    """Создаёт единый RFC822 payload для обоих envelope recipients."""
    message = EmailMessage()
    message["From"] = "Alice <alice@test>"
    message["To"] = "Alice <alice@test>, Bob <bob@test>"
    message["Subject"] = SUBJECT
    message.set_content("One transaction must publish both mailbox links.\n")
    message.add_attachment(
        ATTACHMENT,
        maintype="application",
        subtype="octet-stream",
        filename="atomic.bin",
    )
    return message.as_bytes(policy=email.policy.SMTP)


def submit(host: str, port: int, context: ssl.SSLContext) -> None:
    """Отправляет один DATA с двумя RCPT после AUTH Alice."""
    with smtplib.SMTP_SSL(host, port, context=context, timeout=10) as client:
        client.login("alice", "alice-password")
        refused = client.sendmail(
            "alice@test", ("bob@test", "alice@test"), message_bytes()
        )
    if refused:
        raise AssertionError(f"recipient refusal: {refused!r}")


def extract_literal(records: list[object]) -> bytes:
    """Извлекает literal без позиционного индексирования ответа."""
    for record in records:
        match record:
            case (_, bytes() as payload):
                return payload
            case _:
                continue
    raise AssertionError("IMAP FETCH has no RFC822 literal")


def is_target(raw: bytes) -> bool:
    """Проверяет Subject и точные decoded bytes вложения."""
    parsed = BytesParser(policy=email.policy.default).parsebytes(raw)
    if parsed.get("Subject") != SUBJECT:
        return False
    for part in parsed.iter_attachments():
        if part.get_filename() == "atomic.bin":
            return part.get_payload(decode=True) == ATTACHMENT
    return False


def fetch_for(
    host: str,
    port: int,
    context: ssl.SSLContext,
    account: str,
    password: str,
) -> bytes:
    """Находит transaction в mailbox конкретного получателя."""
    with imaplib.IMAP4_SSL(host, port, ssl_context=context, timeout=10) as client:
        client.login(account, password)
        _, counts = client.select("INBOX", readonly=True)
        count = int(next(iter(counts)))
        for number in range(1, count + 1):
            status, records = client.fetch(str(number), "(RFC822)")
            match ImapStatus(status):
                case ImapStatus.OK:
                    raw = extract_literal(records)
                    if is_target(raw):
                        return raw
    raise AssertionError(f"transaction is absent from {account} mailbox")


def main() -> None:
    """Проверяет видимость и идентичность payload у Alice и Bob."""
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca_file)
    match options.action:
        case Action.SUBMIT:
            submit(options.host, options.smtp_port, context)
        case Action.VERIFY:
            pass
    bob = fetch_for(options.host, options.imap_port, context, "bob", "bob-password")
    alice = fetch_for(
        options.host, options.imap_port, context, "alice", "alice-password"
    )
    if bob != alice:
        raise AssertionError("mailbox links returned different RFC822 bytes")
    print("TLS atomic multi-recipient delivery passed")


if __name__ == "__main__":
    main()
