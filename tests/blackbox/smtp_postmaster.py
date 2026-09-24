"""Проверяет обязательную SMTP доставку Postmaster из RFC 5321."""

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


def path_command(client, verb, address):
    client.send(f"{verb} <{address}>\r\n".encode("ascii"))
    return client.getreply()


def main():
    tls_context = ssl.create_default_context()
    tls_context.check_hostname = False
    tls_context.verify_mode = ssl.CERT_NONE
    with smtplib.SMTP(HOST, PORT, timeout=15) as client:
        client.starttls(context=tls_context)
        expect_reply(client.ehlo(), 250, "EHLO after STARTTLS")
        client.login("alice", os.environ["LITTERKEEP_ALICE_PASSWORD"])
        expect_reply(client.mail("alice@test"), 250, "MAIL FROM")
        expect_reply(path_command(client, "RCPT TO:", "Postmaster"), 250, "bare Postmaster")
        expect_reply(
            path_command(client, "RCPT TO:", "pOsTmAsTeR@TEST"),
            250,
            "qualified Postmaster with DNS case variation",
        )
        expect_reply(
            path_command(client, "RCPT TO:", "Postmaster@example.test"),
            550,
            "unserved Postmaster domain",
        )
        expect_reply(client.docmd("DATA"), 354, "DATA for accepted Postmaster")
        client.send(
            b"From: alice@test\r\nTo: Postmaster\r\n\r\n"
            b"SMTP Postmaster acceptance.\r\n.\r\n"
        )
        expect_reply(client.getreply(), 250, "durable Postmaster delivery")

    print("PASS smtp-postmaster: bare and local-domain Postmaster accepted and delivered")


if __name__ == "__main__":
    main()
