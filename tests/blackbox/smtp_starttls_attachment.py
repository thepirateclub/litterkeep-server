"""Проверяет стандартный SMTP STARTTLS и точность локальной доставки вложения."""

import argparse
import email.policy
import hashlib
import imaplib
import smtplib
import ssl
from email.message import EmailMessage
from email.parser import BytesParser


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--smtp-port", type=int, required=True)
    parser.add_argument("--imap-port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    return parser.parse_args()


def main() -> None:
    options = arguments()
    attachment = bytes(range(256)) * 3
    message = EmailMessage()
    message["From"] = "alice@litterkeep.test"
    message["To"] = "bob@litterkeep.test"
    message["Subject"] = "STARTTLS attachment"
    message.set_content("transport upgrade acceptance")
    message.add_attachment(
        attachment,
        maintype="application",
        subtype="octet-stream",
        filename="starttls.bin",
    )
    context = ssl.create_default_context(cafile=options.ca)
    with smtplib.SMTP("127.0.0.1", options.smtp_port, timeout=10) as client:
        client.ehlo()
        client.starttls(context=context)
        client.ehlo()
        client.login("alice", "alice-password")
        client.send_message(message)
    mailbox = imaplib.IMAP4("127.0.0.1", options.imap_port)
    mailbox.login("bob", "bob-password")
    _, counts = mailbox.select("INBOX", readonly=True)
    sequence = next(value.decode("ascii") for value in counts if isinstance(value, bytes))
    _, payloads = mailbox.fetch(sequence, "(RFC822)")
    raw = next(item for item in payloads if isinstance(item, tuple))
    raw_bytes = next(value for value in reversed(raw) if isinstance(value, bytes))
    parsed = BytesParser(policy=email.policy.default).parsebytes(raw_bytes)
    actual = next(part.get_payload(decode=True) for part in parsed.iter_attachments())
    if actual != attachment:
        raise AssertionError("STARTTLS attachment bytes differ")
    print(f"STARTTLS attachment SHA256={hashlib.sha256(actual).hexdigest()}")


if __name__ == "__main__":
    main()
