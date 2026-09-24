"""Проверяет RFC 1870 SIZE advertisement, параметры и ранний отказ MAIL."""

import os
import imaplib
import re
import smtplib
import ssl
import uuid


HOST = os.environ.get("LITTERKEEP_SMTP_HOST", "127.0.0.1")
PORT = int(os.environ.get("LITTERKEEP_SMTP_PORT", "2525"))
MAXIMUM = 25 * 1024 * 1024


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
    with smtplib.SMTP(HOST, PORT, timeout=10) as client:
        expect_reply(
            client.docmd("MAIL", "FROM:<alice@test>"),
            503,
            "MAIL requires HELO or EHLO",
        )
        client.starttls(context=tls_context)
        code, capabilities = client.ehlo()
        advertised = (f"SIZE {MAXIMUM}".encode("ascii"), b"8BITMIME")
        if code != 250 or any(item not in capabilities for item in advertised):
            raise AssertionError(f"SMTP capabilities absent: {code} {capabilities!r}")
        client.login("alice", os.environ["LITTERKEEP_ALICE_PASSWORD"])

        expect_reply(client.mail("alice@test"), 250, "MAIL before repeated HELO")
        expect_reply(client.rcpt("bob@test"), 250, "RCPT before repeated HELO")
        expect_reply(client.helo("basic.example"), 250, "basic HELO")
        expect_reply(
            client.rcpt("bob@test"),
            503,
            "repeated HELO clears the previous transaction",
        )
        expect_reply(
            client.docmd("MAIL", "FROM:<alice@test> SIZE=1"),
            555,
            "ESMTP parameter requires EHLO",
        )
        expect_reply(
            client.docmd("AUTH", "PLAIN invalid"),
            503,
            "AUTH requires EHLO",
        )
        expect_reply(client.mail("alice@test"), 250, "basic SMTP MAIL")
        expect_reply(client.rcpt("bob@test"), 250, "basic SMTP RCPT")
        expect_reply(client.rset(), 250, "RSET from basic greeting")

        expect_reply(client.ehlo("extended.example"), 250, "extended EHLO")
        expect_reply(client.mail("alice@test"), 250, "MAIL before repeated EHLO")
        expect_reply(client.ehlo("reset.example"), 250, "repeated EHLO")
        expect_reply(
            client.rcpt("bob@test"),
            503,
            "repeated EHLO clears the previous transaction",
        )
        expect_reply(client.noop(), 250, "session after EHLO reset")

        expect_reply(
            client.mail("alice@test", options=[f"SIZE={MAXIMUM + 1}"]),
            552,
            "oversized declaration",
        )
        expect_reply(client.noop(), 250, "session after oversized MAIL")

        expect_reply(
            client.docmd("MAIL", "FROM:<alice@test> SIZE=not-a-number"),
            501,
            "malformed SIZE",
        )
        expect_reply(
            client.docmd("MAIL", "FROM:<alice@test> SIZE=1 SIZE=2"),
            501,
            "duplicate SIZE",
        )
        expect_reply(
            client.docmd("MAIL", "FROM:<alice@test> FOO=BAR"),
            555,
            "unsupported MAIL parameter",
        )

        expect_reply(
            client.mail("alice@test", options=[f"SIZE={MAXIMUM}"]),
            250,
            "maximum supported declaration",
        )
        expect_reply(
            client.docmd("RCPT", "TO:<bob@test> NOTIFY=SUCCESS"),
            555,
            "unsupported RCPT parameter",
        )
        expect_reply(client.rset(), 250, "reset after accepted MAIL")

        expect_reply(client.docmd("VRFY", "alice@test"), 252, "privacy-safe VRFY")
        expect_reply(client.docmd("EXPN", "staff"), 502, "disabled EXPN")
        help_reply = client.docmd("HELP")
        expect_reply(help_reply, 214, "HELP command list")
        if b"VRFY" not in help_reply[1] or b"QUIT" not in help_reply[1]:
            raise AssertionError(f"HELP omits supported SMTP commands: {help_reply!r}")
        expect_reply(client.docmd("NOOP", "ignored argument"), 250, "NOOP parameter")

        expect_reply(client.mail("alice@test"), 250, "MAIL before verify commands")
        expect_reply(client.docmd("VRFY", "bob@test"), 252, "VRFY during envelope")
        expect_reply(client.rcpt("bob@test"), 250, "VRFY preserves active envelope")
        expect_reply(client.rset(), 250, "RSET after verify commands")

        expect_reply(client.mail("alice@test"), 250, "MAIL without SIZE")
        expect_reply(client.rcpt("bob@test"), 250, "RCPT before oversized DATA")
        expect_reply(client.docmd("DATA"), 354, "DATA before oversized body")
        client.send(("x" * (MAXIMUM + 1) + "\r\n.\r\n").encode("ascii"))
        expect_reply(client.getreply(), 552, "oversized DATA body")
        expect_reply(client.noop(), 250, "session after oversized DATA")

        high_bit_subject = "8BITMIME " + uuid.uuid4().hex
        high_bit_body = "Здравствуй, почта!\r\n".encode("utf-8")
        raw_message = (
            b"From: alice@test\r\nTo: bob@test\r\nSubject: "
            + high_bit_subject.encode("ascii")
            + b"\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n"
            b"Content-Transfer-Encoding: 8bit\r\n\r\n"
            + high_bit_body
        )
        expect_reply(client.mail("alice@test", options=["BODY=8BITMIME"]), 250, "8BITMIME MAIL")
        expect_reply(client.rcpt("bob@test"), 250, "8BITMIME RCPT")
        expect_reply(client.docmd("DATA"), 354, "8BITMIME DATA")
        client.send(raw_message + b".\r\n")
        expect_reply(client.getreply(), 250, "8BITMIME delivery")

        expect_reply(client.mail("alice@test"), 250, "7BIT MAIL")
        expect_reply(client.rcpt("bob@test"), 250, "7BIT RCPT")
        expect_reply(client.docmd("DATA"), 354, "7BIT DATA")
        client.send(b"From: alice@test\r\nTo: bob@test\r\n\r\n" + high_bit_body + b".\r\n")
        expect_reply(client.getreply(), 552, "8-bit body without BODY=8BITMIME")
        expect_reply(client.noop(), 250, "session after undeclared 8-bit body")

    imap_context = ssl.create_default_context()
    imap_context.check_hostname = False
    imap_context.verify_mode = ssl.CERT_NONE
    with imaplib.IMAP4(HOST, 2143) as mailbox:
        mailbox.starttls(ssl_context=imap_context)
        mailbox.login("bob", os.environ["LITTERKEEP_BOB_PASSWORD"])
        select_status, _ = mailbox.select("INBOX")
        if select_status != "OK":
            raise AssertionError(f"IMAP SELECT failed: {select_status}")
        status, rows = mailbox.search(None, "HEADER", "Subject", f'"{high_bit_subject}"')
        first_row = next(iter(rows), None)
        if status != "OK" or first_row is None:
            raise AssertionError(f"8BITMIME message not found in IMAP: {status} {rows!r}")
        sequence = next(reversed(first_row.split())).decode("ascii")
        status, fetched = mailbox.fetch(sequence, "(BODY.PEEK[TEXT])")
        if status != "OK":
            raise AssertionError(f"IMAP FETCH failed: {status} {fetched!r}")
        literal = next((item[1] for item in fetched if isinstance(item, tuple)), None)
        if literal is None or high_bit_body not in literal:
            raise AssertionError("8BITMIME octets changed between SMTP and IMAP")
        if re.search(rb"[\x80-\xff]", literal) is None:
            raise AssertionError("IMAP body unexpectedly contains only 7-bit octets")
    print("PASS smtp-size-declaration: SIZE, DATA limits, 8BITMIME round trip, 7BIT rejection")


if __name__ == "__main__":
    main()
