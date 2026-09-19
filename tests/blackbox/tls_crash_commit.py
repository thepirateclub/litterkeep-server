"""Двухфазная проверка сохранности подтверждённого SMTP commit после kill."""

import argparse
import email.policy
import imaplib
import smtplib
import ssl
from email.message import EmailMessage
from email.parser import BytesParser
from enum import Enum


ATTACHMENT = b"durable-after-process-kill\x00\xff"
SUBJECT = "durable crash acceptance 2026-09-19"


class Action(Enum):
    """Фаза теста, между которыми внешний runner завершает процесс."""

    SUBMIT = "submit"
    VERIFY = "verify"


class ImapStatus(Enum):
    """Типизированный результат команды независимого IMAP-клиента."""

    OK = "OK"


def arguments() -> argparse.Namespace:
    """Читает фазу, endpoint и тестовый CA."""
    parser = argparse.ArgumentParser()
    parser.add_argument("action", type=Action)
    parser.add_argument("host")
    parser.add_argument("port", type=int)
    parser.add_argument("ca_file")
    return parser.parse_args()


def message_bytes() -> bytes:
    """Создаёт различимое RFC822-письмо с бинарным доказательством."""
    message = EmailMessage()
    message["From"] = "Alice <alice@test>"
    message["To"] = "Bob <bob@test>"
    message["Subject"] = SUBJECT
    message.set_content("The SMTP commit must survive process termination.\n")
    message.add_attachment(
        ATTACHMENT,
        maintype="application",
        subtype="octet-stream",
        filename="crash-proof.bin",
    )
    return message.as_bytes(policy=email.policy.SMTP)


def submit(host: str, port: int, context: ssl.SSLContext) -> None:
    """Ждёт финальный SMTP success до завершения первой фазы."""
    with smtplib.SMTP_SSL(host, port, context=context, timeout=10) as client:
        client.login("alice", "alice-password")
        refused = client.sendmail("alice@test", ("bob@test",), message_bytes())
    if refused:
        raise AssertionError(f"recipient was refused: {refused!r}")
    print("durable crash message accepted by SMTP")


def literal(records: list[object]) -> bytes:
    """Извлекает RFC822 literal без позиционного доступа к ответу IMAP."""
    for record in records:
        match record:
            case (_, bytes() as payload):
                return payload
            case _:
                continue
    raise AssertionError("IMAP response has no RFC822 literal")


def contains_evidence(raw: bytes) -> bool:
    """Проверяет одновременно Subject и точные байты MIME-вложения."""
    parsed = BytesParser(policy=email.policy.default).parsebytes(raw)
    if parsed.get("Subject") != SUBJECT:
        return False
    for part in parsed.iter_attachments():
        match part.get_filename():
            case "crash-proof.bin":
                return part.get_payload(decode=True) == ATTACHMENT
            case _:
                continue
    return False


def verify(host: str, port: int, context: ssl.SSLContext) -> None:
    """После нового процесса находит прежний commit через TLS IMAP."""
    with imaplib.IMAP4_SSL(host, port, ssl_context=context, timeout=10) as client:
        client.login("bob", "bob-password")
        _, counts = client.select("INBOX", readonly=True)
        count = int(next(iter(counts)))
        for number in range(1, count + 1):
            status, records = client.fetch(str(number), "(RFC822)")
            match (ImapStatus(status), contains_evidence(literal(records))):
                case (ImapStatus.OK, True):
                    print("durable crash message survived process restart")
                    return
                case (ImapStatus.OK, False):
                    continue
    raise AssertionError("confirmed SMTP commit disappeared after restart")


def main() -> None:
    """Исполняет выбранную фазу с полной проверкой TLS trust и hostname."""
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca_file)
    match options.action:
        case Action.SUBMIT:
            submit(options.host, options.port, context)
        case Action.VERIFY:
            verify(options.host, options.port, context)


if __name__ == "__main__":
    main()
