"""Проверяет 100 получателей SMTP и ответ на 101-й RCPT."""

import imaplib
import os
import smtplib
import ssl
import uuid


HOST = os.environ.get("LITTERKEEP_SMTP_HOST", "127.0.0.1")
SMTP_PORT = int(os.environ.get("LITTERKEEP_SMTP_PORT", "2525"))
IMAP_PORT = int(os.environ.get("LITTERKEEP_IMAP_PORT", "2143"))
MAX_RECIPIENTS = 100


def expect_reply(actual, expected, context):
    code, response = actual
    if code != expected:
        raise AssertionError(
            f"{context}: expected {expected}, received {code} {response!r}"
        )


def main():
    tls_context = ssl.create_default_context()
    tls_context.check_hostname = False
    tls_context.verify_mode = ssl.CERT_NONE
    subject = "Recipient limit " + uuid.uuid4().hex
    message = (
        b"From: alice@test\r\nTo: bob@test\r\nSubject: "
        + subject.encode("ascii")
        + b"\r\n\r\n100 accepted recipients survive RCPT 101.\r\n"
    )

    with smtplib.SMTP(HOST, SMTP_PORT, timeout=20) as client:
        client.starttls(context=tls_context)
        expect_reply(client.ehlo(), 250, "EHLO after STARTTLS")
        client.login("alice", os.environ["LITTERKEEP_ALICE_PASSWORD"])
        expect_reply(client.mail("alice@test"), 250, "MAIL FROM")
        for recipient_number in range(MAX_RECIPIENTS):
            expect_reply(
                client.rcpt("bob@test"), 250, f"accepted RCPT {recipient_number + 1}"
            )
        expect_reply(client.rcpt("qa@test"), 452, "implementation limit RCPT 101")
        expect_reply(client.docmd("DATA"), 354, "DATA preserves accepted recipients")
        client.send(message + b".\r\n")
        expect_reply(client.getreply(), 250, "delivery to accepted recipient set")

    with imaplib.IMAP4(HOST, IMAP_PORT) as mailbox:
        mailbox.starttls(ssl_context=tls_context)
        mailbox.login("bob", os.environ["LITTERKEEP_BOB_PASSWORD"])
        select_status, _ = mailbox.select("INBOX")
        if select_status != "OK":
            raise AssertionError(f"IMAP SELECT failed: {select_status}")
        status, rows = mailbox.search(None, "HEADER", "Subject", f'"{subject}"')
        if status != "OK" or not next(iter(rows), None):
            raise AssertionError(f"accepted recipient lost after 452: {status} {rows!r}")

    print("PASS smtp-recipient-limit: 100 RCPT accepted, 101st gets 452, prior recipient delivered")


if __name__ == "__main__":
    main()
