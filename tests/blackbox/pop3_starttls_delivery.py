"""Независимая SMTP -> POP3/STLS проверка одного durable INBOX."""

import argparse
from email.message import EmailMessage
from email.parser import BytesParser
from email.policy import default
import poplib
import smtplib
import ssl


ATTACHMENT = b"\x00litterkeep-pop3-attachment\xff\x10"


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=("smtp", "pop3", "empty"), required=True)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    return parser.parse_args()


def context(ca: str) -> ssl.SSLContext:
    return ssl.create_default_context(cafile=ca)


def smtp(port: int, tls: ssl.SSLContext) -> None:
    message = EmailMessage()
    message["From"] = "alice@litterkeep.test"
    message["To"] = "bob@litterkeep.test"
    message["Subject"] = "POP3 durable attachment"
    message.set_content("POP3 acceptance message")
    message.add_attachment(
        ATTACHMENT,
        maintype="application",
        subtype="octet-stream",
        filename="proof.bin",
    )
    with smtplib.SMTP("127.0.0.1", port) as client:
        client.starttls(context=tls)
        client.login("alice", "alice-password")
        client.send_message(message)
    print("SMTP delivery passed")


def pop3(port: int, tls: ssl.SSLContext) -> None:
    client = poplib.POP3("127.0.0.1", port)
    client.stls(context=tls)
    client.user("bob")
    client.pass_("bob-password")
    count, _ = client.stat()
    if count != 1:
        raise AssertionError(f"expected one delivered message, got {count}")
    _, uidl_lines, _ = client.uidl()
    if b"lk-" not in b"\n".join(uidl_lines):
        raise AssertionError(f"durable UIDL missing: {uidl_lines!r}")
    _, lines, _ = client.retr(1)
    parsed = BytesParser(policy=default).parsebytes(b"\r\n".join(lines) + b"\r\n")
    attachments = tuple(parsed.iter_attachments())
    attachment = next(iter(attachments), None)
    if attachment is None or attachment.get_payload(decode=True) != ATTACHMENT:
        raise AssertionError("POP3 RETR attachment bytes differ")
    client.dele(1)
    client.quit()
    print("POP3 STLS passed: UIDL, RETR attachment bytes and durable QUIT")


def empty(port: int, tls: ssl.SSLContext) -> None:
    client = poplib.POP3("127.0.0.1", port)
    client.stls(context=tls)
    client.user("bob")
    client.pass_("bob-password")
    count, _ = client.stat()
    client.quit()
    if count != 0:
        raise AssertionError(f"QUIT deletion did not survive restart: {count}")
    print("POP3 restart replay passed")


def main() -> None:
    options = arguments()
    tls = context(options.ca)
    match options.phase:
        case "smtp":
            smtp(options.port, tls)
        case "pop3":
            pop3(options.port, tls)
        case "empty":
            empty(options.port, tls)


if __name__ == "__main__":
    main()
