"""Проверяет 64-октетный local-part в SMTP MAIL и RCPT."""

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
    maximum_address = f"{'u' * 64}@test"
    overlong_address = f"{'u' * 65}@test"

    with smtplib.SMTP(HOST, PORT, timeout=10) as client:
        client.starttls(context=tls_context)
        expect_reply(client.ehlo(), 250, "EHLO after STARTTLS")
        client.login("alice", os.environ["LITTERKEEP_ALICE_PASSWORD"])

        expect_reply(client.mail(maximum_address), 553, "64-octet sender parses")
        expect_reply(client.mail(overlong_address), 501, "65-octet sender is rejected")
        expect_reply(client.mail("alice@test"), 250, "valid sender after path errors")
        expect_reply(client.rcpt(maximum_address), 550, "64-octet recipient parses")
        expect_reply(client.docmd("RCPT", f"TO:<{overlong_address}>"), 501,
                     "65-octet recipient is rejected")
        expect_reply(client.rset(), 250, "session after local-part checks")

    print("PASS smtp-local-part-length: 64 octets accepted, 65 rejected for MAIL/RCPT")


if __name__ == "__main__":
    main()
