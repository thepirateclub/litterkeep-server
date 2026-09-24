"""Проверяет RFC 5321 path limit для MAIL FROM и RCPT TO."""

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
    accepted_domain = ".".join(("a" * 63, "b" * 63, "c" * 61))
    overlong_domain = ".".join(("a" * 63, "b" * 63, "c" * 62))
    local_part = "u" * 64
    accepted_address = f"{local_part}@{accepted_domain}"
    overlong_address = f"{local_part}@{overlong_domain}"

    with smtplib.SMTP(HOST, PORT, timeout=10) as client:
        client.starttls(context=tls_context)
        expect_reply(client.ehlo(), 250, "EHLO after STARTTLS")
        client.login("alice", os.environ["LITTERKEEP_ALICE_PASSWORD"])

        expect_reply(client.mail(accepted_address), 553, "256-octet reverse path parses")
        expect_reply(
            client.mail(overlong_address), 501, "257-octet reverse path is rejected"
        )
        expect_reply(client.mail("alice@test"), 250, "valid sender after MAIL errors")
        expect_reply(client.rcpt(accepted_address), 550, "256-octet forward path parses")
        expect_reply(
            client.rcpt(overlong_address), 501, "257-octet forward path is rejected"
        )
        expect_reply(client.rset(), 250, "session after path-limit checks")

    print("PASS smtp-path-length: 256-octet paths accepted by parser, 257 rejected")


if __name__ == "__main__":
    main()
