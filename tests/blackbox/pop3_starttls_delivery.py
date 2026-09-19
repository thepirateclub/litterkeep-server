"""Независимая SMTP -> POP3/STLS проверка одного durable INBOX."""

import argparse
from email.message import EmailMessage
from email.parser import BytesParser
from email.policy import default
import poplib
import smtplib
import ssl
import time


ATTACHMENT = b"\x00litterkeep-pop3-attachment\xff\x10"


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=("smtp", "pop3", "empty", "lease", "implicit", "rset", "disconnect"), required=True)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--account", default="bob")
    parser.add_argument("--password", default="bob-password")
    return parser.parse_args()


def context(ca: str) -> ssl.SSLContext:
    return ssl.create_default_context(cafile=ca)


def smtp(port: int, tls: ssl.SSLContext, account: str) -> None:
    message = EmailMessage()
    message["From"] = "alice@litterkeep.test"
    message["To"] = f"{account}@litterkeep.test"
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


def pop3(port: int, tls: ssl.SSLContext, account: str, password: str) -> None:
    client = poplib.POP3("127.0.0.1", port)
    client.stls(context=tls)
    client.user(account)
    client.pass_(password)
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


def empty(port: int, tls: ssl.SSLContext, account: str, password: str) -> None:
    client = poplib.POP3("127.0.0.1", port)
    client.stls(context=tls)
    client.user(account)
    client.pass_(password)
    count, _ = client.stat()
    client.quit()
    if count != 0:
        raise AssertionError(f"QUIT deletion did not survive restart: {count}")
    print("POP3 restart replay passed")


def lease(port: int, tls: ssl.SSLContext, account: str, password: str) -> None:
    first = authenticated(port, tls, account, password)
    second = poplib.POP3("127.0.0.1", port)
    second.stls(context=tls)
    second.user(account)
    try:
        second.pass_(password)
    except poplib.error_proto as error:
        if "maildrop is locked" not in str(error):
            raise
    else:
        raise AssertionError("concurrent POP3 session received the same maildrop")
    first.close()
    time.sleep(0.2)
    second.pass_(password)
    second.quit()
    print("POP3 maildrop lease passed: concurrent denial and EOF release")


def implicit(port: int, tls: ssl.SSLContext, account: str, password: str) -> None:
    client = poplib.POP3_SSL("127.0.0.1", port, context=tls)
    client.user(account)
    client.pass_(password)
    client.quit()
    print("POP3 implicit TLS passed")


def authenticated(port: int, tls: ssl.SSLContext, account: str, password: str) -> poplib.POP3:
    client = poplib.POP3("127.0.0.1", port)
    client.stls(context=tls)
    client.user(account)
    client.pass_(password)
    return client


def rset(port: int, tls: ssl.SSLContext, account: str, password: str) -> None:
    client = authenticated(port, tls, account, password)
    client.dele(1)
    client.rset()
    client.quit()
    replay = authenticated(port, tls, account, password)
    count, _ = replay.stat()
    replay.quit()
    if count != 1:
        raise AssertionError(f"RSET failed to retain the message: {count}")
    print("POP3 RSET passed: DELE marker was not committed")


def disconnect(port: int, tls: ssl.SSLContext, account: str, password: str) -> None:
    client = authenticated(port, tls, account, password)
    client.dele(1)
    client.close()
    time.sleep(0.2)
    replay = authenticated(port, tls, account, password)
    count, _ = replay.stat()
    replay.quit()
    if count != 1:
        raise AssertionError(f"disconnect committed a POP3 DELE marker: {count}")
    print("POP3 disconnect passed: DELE marker was not committed")


def main() -> None:
    options = arguments()
    tls = context(options.ca)
    match options.phase:
        case "smtp":
            smtp(options.port, tls, options.account)
        case "pop3":
            pop3(options.port, tls, options.account, options.password)
        case "empty":
            empty(options.port, tls, options.account, options.password)
        case "lease":
            lease(options.port, tls, options.account, options.password)
        case "implicit":
            implicit(options.port, tls, options.account, options.password)
        case "rset":
            rset(options.port, tls, options.account, options.password)
        case "disconnect":
            disconnect(options.port, tls, options.account, options.password)


if __name__ == "__main__":
    main()
