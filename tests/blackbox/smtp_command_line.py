"""Проверяет базовый RFC 5321 предел SMTP-командных строк."""

import os
import smtplib
import ssl


HOST = os.environ.get("LITTERKEEP_SMTP_HOST", "127.0.0.1")
PORT = int(os.environ.get("LITTERKEEP_SMTP_PORT", "2525"))


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
        expect_reply(client.ehlo(), 250, "EHLO after STARTTLS")

        client.send(b"NOOP " + b"x" * 505 + b"\r\n")
        expect_reply(client.getreply(), 250, "512-octet command line")
        client.send(b"NOOP " + b"x" * 506 + b"\r\n")
        expect_reply(client.getreply(), 500, "513-octet command line")
        expect_reply(client.noop(), 250, "command channel after rejected line")

    print("PASS smtp-command-line: 512 accepted, 513 rejected, next command synchronized")


if __name__ == "__main__":
    main()
