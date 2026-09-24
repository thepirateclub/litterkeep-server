"""Проверяет RFC 1870 SIZE advertisement, параметры и ранний отказ MAIL."""

import os
import smtplib
import ssl


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
        client.starttls(context=tls_context)
        code, capabilities = client.ehlo()
        if code != 250 or f"SIZE {MAXIMUM}".encode("ascii") not in capabilities:
            raise AssertionError(f"SIZE capability absent: {code} {capabilities!r}")
        client.login("alice", os.environ["LITTERKEEP_ALICE_PASSWORD"])

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
            client.docmd("MAIL", "FROM:<alice@test> BODY=8BITMIME"),
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

        expect_reply(client.mail("alice@test"), 250, "MAIL without SIZE")
        expect_reply(client.rcpt("bob@test"), 250, "RCPT before oversized DATA")
        expect_reply(client.docmd("DATA"), 354, "DATA before oversized body")
        client.send(("x" * (MAXIMUM + 1) + "\r\n.\r\n").encode("ascii"))
        expect_reply(client.getreply(), 552, "oversized DATA body")
        expect_reply(client.noop(), 250, "session after oversized DATA")
    print("PASS smtp-size-declaration: SIZE, 552, 501, 555, session recovery")


if __name__ == "__main__":
    main()
