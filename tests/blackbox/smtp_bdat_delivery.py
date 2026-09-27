"""Проверяет многокусковую SMTP BDAT доставку и байты MIME-вложения."""

import argparse
import email.policy
import hashlib
import imaplib
import smtplib
import ssl
from email.message import EmailMessage
from email.parser import BytesParser
from enum import Enum, IntEnum
from pathlib import Path


ATTACHMENT = b"BDAT\x00chunk\r\nbytes\xff"


class ReplyCode(IntEnum):
    """Коды SMTP, нужные тесту последовательности BDAT."""

    ACCEPTED = 250


class ImapStatus(Enum):
    """Успешный ответ IMAP независимого клиента."""

    OK = "OK"


class AttachmentFilename(str, Enum):
    """Имя MIME части, доставку которой подтверждает acceptance."""

    BDAT = "bdat.bin"


def arguments() -> argparse.Namespace:
    """Разбирает endpoints, CA и файл тестовых учётных данных."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("smtp_port", type=int)
    parser.add_argument("imap_port", type=int)
    parser.add_argument("ca_file", type=Path)
    parser.add_argument("credentials", type=Path)
    return parser.parse_args()


def passwords(path: Path) -> dict[str, str]:
    """Читает bootstrap TSV, не выводя пароли в журнал."""
    result: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t"):
            case [account, secret]:
                result[account] = secret
    return result


def message() -> bytes:
    """Строит MIME письмо со вложением, имеющим NUL и CRLF октеты."""
    value = EmailMessage()
    value["From"] = "Alice <alice@litterkeep.test>"
    value["To"] = "Bob <bob@litterkeep.test>"
    value["Subject"] = "RFC 3030 BDAT delivery"
    value.set_content("MIME transport through chunked BDAT.\n")
    value.add_attachment(
        ATTACHMENT,
        maintype="application",
        subtype="octet-stream",
        filename="bdat.bin",
    )
    return value.as_bytes(policy=email.policy.SMTP)


def require_accepted(client: smtplib.SMTP, phase: str) -> None:
    """Проверяет SMTP ответ без подавления кода и диагностического текста."""
    code, response = client.getreply()
    if code != ReplyCode.ACCEPTED:
        raise AssertionError(f"{phase}: SMTP {code} {response!r}")


def submit(host: str, port: int, context: ssl.SSLContext, secret: str) -> None:
    """Отправляет письмо двумя кусками, второй завершается маркером LAST."""
    payload = message()
    split = len(payload) // 2
    with smtplib.SMTP_SSL(host, port, context=context, timeout=15) as client:
        client.login("alice", secret)
        client.putcmd("MAIL", "FROM:<alice@litterkeep.test>")
        require_accepted(client, "MAIL")
        client.putcmd("RCPT", "TO:<bob@litterkeep.test>")
        require_accepted(client, "RCPT")
        client.putcmd("BDAT", "0")
        require_accepted(client, "zero-length intermediate BDAT")
        first = payload[:split]
        client.putcmd("BDAT", str(len(first)))
        client.send(first)
        require_accepted(client, "intermediate BDAT")
        final = payload[split:]
        client.putcmd("BDAT", f"{len(final)} LAST")
        client.send(final)
        require_accepted(client, "BDAT LAST")


def delivered_attachment(
    host: str, port: int, context: ssl.SSLContext, secret: str
) -> bytes:
    """Получает сохранённое письмо Bob через независимый IMAP TLS клиент."""
    with imaplib.IMAP4_SSL(host, port, ssl_context=context, timeout=15) as client:
        client.login("bob", secret)
        status, counts = client.select("INBOX", readonly=True)
        if ImapStatus(status) is not ImapStatus.OK:
            raise AssertionError("cannot select Bob INBOX")
        for count in counts:
            total = int(count)
            for number in range(1, total + 1):
                status, response = client.fetch(str(number), "(RFC822)")
                if ImapStatus(status) is not ImapStatus.OK:
                    continue
                for item in response:
                    match item:
                        case (_, bytes() as raw):
                            parsed = BytesParser(
                                policy=email.policy.default
                            ).parsebytes(raw)
                        case _:
                            continue
                    for part in parsed.iter_attachments():
                        if part.get_filename() == AttachmentFilename.BDAT:
                            payload = part.get_payload(decode=True)
                            if isinstance(payload, bytes):
                                return payload
    raise AssertionError("BDAT attachment was not found through IMAP")


def main() -> None:
    """Сверяет исходные и прочитанные через IMAP octets вложения."""
    options = arguments()
    context = ssl.create_default_context(cafile=str(options.ca_file))
    credentials = passwords(options.credentials)
    alice_password = credentials.get("alice")
    bob_password = credentials.get("bob")
    if alice_password is None or bob_password is None:
        raise AssertionError("bootstrap credentials must contain Alice and Bob")
    submit(options.host, options.smtp_port, context, alice_password)
    actual = delivered_attachment(
        options.host, options.imap_port, context, bob_password
    )
    if actual != ATTACHMENT:
        raise AssertionError("MIME attachment octets changed during BDAT delivery")
    digest = hashlib.sha256(actual).hexdigest()
    print(f"SMTP BDAT to IMAP attachment passed: sha256={digest}")


if __name__ == "__main__":
    main()
