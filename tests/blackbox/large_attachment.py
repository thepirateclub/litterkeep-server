"""Проверяет SMTP -> Store -> IMAP для точного вложения размером 10 MiB."""

import argparse
import email.policy
import hashlib
import imaplib
import smtplib
import ssl
from email.message import EmailMessage
from email.parser import BytesParser
from enum import Enum


ATTACHMENT_SIZE = 10 * 1024 * 1024
SUBJECT = "litterkeep 10 MiB attachment"


class ImapStatus(Enum):
    """Допустимый типизированный статус независимого IMAP-клиента."""

    OK = "OK"


def arguments() -> argparse.Namespace:
    """Читает оба endpoint и CA без скрытой конфигурации."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("smtp_port", type=int)
    parser.add_argument("imap_port", type=int)
    parser.add_argument("ca_file")
    parser.add_argument("--attachment-bytes", type=int, default=ATTACHMENT_SIZE)
    return parser.parse_args()


def attachment_bytes(size: int) -> bytes:
    """Строит воспроизводимый бинарный payload всех значений октета."""
    pattern = bytes(range(256))
    repeats, remainder = divmod(size, len(pattern))
    if remainder != 0:
        raise AssertionError("fixture size must contain complete octet patterns")
    return pattern * repeats


def message(payload: bytes) -> bytes:
    """Создаёт MIME с base64 overhead поверх требуемых 10 MiB."""
    value = EmailMessage()
    value["From"] = "alice@litterkeep.test"
    value["To"] = "bob@litterkeep.test"
    value["Subject"] = SUBJECT
    value.set_content("Large attachment acceptance.\n")
    value.add_attachment(
        payload,
        maintype="application",
        subtype="octet-stream",
        filename="ten-mebibytes.bin",
    )
    return value.as_bytes(policy=email.policy.SMTP)


def literal(records: list[object]) -> bytes:
    """Находит RFC822 literal без позиционного доступа к массиву ответа."""
    for record in records:
        match record:
            case (_, bytes() as payload):
                return payload
            case _:
                continue
    raise AssertionError("IMAP FETCH returned no literal")


def downloaded_attachment(raw: bytes) -> bytes:
    """Извлекает вложение стандартным независимым MIME parser-ом."""
    parsed = BytesParser(policy=email.policy.default).parsebytes(raw)
    for part in parsed.iter_attachments():
        match part.get_filename():
            case "ten-mebibytes.bin":
                return part.get_payload(decode=True)
            case _:
                continue
    raise AssertionError("large attachment is absent")


def main() -> None:
    """Отправляет, скачивает и сравнивает размер и SHA-256 payload."""
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca_file)
    payload = attachment_bytes(options.attachment_bytes)
    raw = message(payload)
    with smtplib.SMTP(options.host, options.smtp_port, timeout=60) as client:
        client.starttls(context=context)
        client.login("alice", "alice-password")
        client.sendmail("alice@litterkeep.test", ("bob@litterkeep.test",), raw)
    with imaplib.IMAP4(options.host, options.imap_port, timeout=60) as client:
        client.starttls(ssl_context=context)
        client.login("bob", "bob-password")
        status, counts = client.select("INBOX", readonly=True)
        if ImapStatus(status) is not ImapStatus.OK:
            raise AssertionError("cannot select INBOX")
        sequence = str(int(next(iter(counts))))
        status, records = client.fetch(sequence, "(RFC822)")
        if ImapStatus(status) is not ImapStatus.OK:
            raise AssertionError("cannot fetch message")
    downloaded = downloaded_attachment(literal(records))
    expected_hash = hashlib.sha256(payload).hexdigest()
    actual_hash = hashlib.sha256(downloaded).hexdigest()
    if len(downloaded) != options.attachment_bytes:
        raise AssertionError(f"wrong attachment size: {len(downloaded)}")
    if actual_hash != expected_hash:
        raise AssertionError("attachment SHA-256 mismatch")
    print(
        f"PASS attachment_bytes={len(downloaded)} raw_bytes={len(raw)} "
        f"sha256={actual_hash}"
    )


if __name__ == "__main__":
    main()
