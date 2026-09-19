"""Проверяет стандартный SMTP STARTTLS и точность локальной доставки вложения."""

import argparse
import contextlib
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


def reject_unknown_imap_ca(port: int) -> None:
    """Доказывает, что STARTTLS-клиент не принимает неизвестную цепочку."""
    client = imaplib.IMAP4("127.0.0.1", port)
    try:
        client.starttls(ssl_context=ssl.create_default_context())
    except ssl.SSLCertVerificationError:
        pass
    else:
        raise AssertionError("IMAP STARTTLS accepted an unknown CA")
    finally:
        with contextlib.suppress(OSError):
            client.shutdown()


def main() -> None:
    options = arguments()
    reject_unknown_imap_ca(options.imap_port)
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
    plaintext = imaplib.IMAP4("127.0.0.1", options.imap_port)
    try:
        plaintext.login("bob", "bob-password")
    except imaplib.IMAP4.error:
        pass
    else:
        raise AssertionError("plaintext IMAP LOGIN was accepted before STARTTLS")
    finally:
        plaintext.shutdown()
    mailbox = imaplib.IMAP4("127.0.0.1", options.imap_port)
    mailbox.starttls(ssl_context=context)
    mailbox.login("bob", "bob-password")
    try:
        mailbox.fetch("1", "(RFC822.SIZE)")
    except imaplib.IMAP4.error:
        pass
    else:
        raise AssertionError("FETCH was accepted before SELECT")
    _, counts = mailbox.select("INBOX", readonly=True)
    uid_validity = mailbox.response("UIDVALIDITY")
    uid_next = mailbox.response("UIDNEXT")
    if uid_validity != ("UIDVALIDITY", [b"1"]):
        raise AssertionError(f"unexpected UIDVALIDITY response: {uid_validity!r}")
    if uid_next != ("UIDNEXT", [b"2"]):
        raise AssertionError(f"unexpected UIDNEXT response: {uid_next!r}")
    sequence = next(value.decode("ascii") for value in counts if isinstance(value, bytes))
    _, uid_fetch = mailbox.uid("FETCH", "1:*", "(RFC822.SIZE)")
    uid_wire = b" ".join(item for item in uid_fetch if isinstance(item, bytes))
    if b"UID 1 RFC822.SIZE" not in uid_wire:
        raise AssertionError(f"UID FETCH did not include UID: {uid_wire!r}")
    _, structures = mailbox.fetch(sequence, "(BODYSTRUCTURE)")
    structure_bytes = b" ".join(
        value
        for item in structures
        for value in (item if isinstance(item, tuple) else (item,))
        if isinstance(value, bytes)
    )
    if b'"MULTIPART"' in structure_bytes:
        raise AssertionError("multipart BODYSTRUCTURE used a scalar type")
    if b'"MIXED"' not in structure_bytes:
        raise AssertionError("multipart/mixed BODYSTRUCTURE is absent")
    _, envelopes = mailbox.fetch(sequence, "(ENVELOPE)")
    envelope_bytes = b" ".join(item for item in envelopes if isinstance(item, bytes))
    for required in (b"STARTTLS attachment", b'"alice" "litterkeep.test"', b'"bob" "litterkeep.test"'):
        if required not in envelope_bytes:
            raise AssertionError("ENVELOPE field is absent")
    _, sizes = mailbox.fetch(sequence, "(RFC822.SIZE)")
    if not any(b"RFC822.SIZE" in item for item in sizes if isinstance(item, bytes)):
        raise AssertionError("RFC822.SIZE response is absent")
    _, partials = mailbox.fetch(sequence, "(BODY.PEEK[1]<0.5>)")
    partial = next(
        value
        for item in partials
        if isinstance(item, tuple)
        for value in reversed(item)
        if isinstance(value, bytes)
    )
    if partial != b"trans":
        raise AssertionError("BODY partial is not measured over raw part octets")
    _, mime_sections = mailbox.fetch(sequence, "(BODY.PEEK[2.MIME])")
    mime_section = next(
        value
        for item in mime_sections
        if isinstance(item, tuple)
        for value in reversed(item)
        if isinstance(value, bytes)
    )
    if not mime_section.startswith(b"Content-Type: application/octet-stream"):
        raise AssertionError("BODY part MIME headers are incorrect")
    _, payloads = mailbox.fetch(sequence, "(RFC822)")
    raw = next(item for item in payloads if isinstance(item, tuple))
    raw_bytes = next(value for value in reversed(raw) if isinstance(value, bytes))
    if not raw_bytes.startswith(b"Return-Path: <alice@litterkeep.test>\r\nReceived: "):
        raise AssertionError("delivery trace headers are absent or use the wrong envelope")
    parsed = BytesParser(policy=email.policy.default).parsebytes(raw_bytes)
    actual = next(part.get_payload(decode=True) for part in parsed.iter_attachments())
    if actual != attachment:
        raise AssertionError("STARTTLS attachment bytes differ")
    print(f"STARTTLS attachment SHA256={hashlib.sha256(actual).hexdigest()}")


if __name__ == "__main__":
    main()
