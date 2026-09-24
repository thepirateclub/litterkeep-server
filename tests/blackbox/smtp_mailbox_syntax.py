"""Проверяет dot-atom mailbox grammar SMTP MAIL и RCPT."""

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


def send_path_command(client, verb, mailbox):
    command = f"{verb} <{mailbox}>\r\n".encode("utf-8")
    client.send(command)
    return client.getreply()


def main():
    tls_context = ssl.create_default_context()
    tls_context.check_hostname = False
    tls_context.verify_mode = ssl.CERT_NONE
    invalid_mailboxes = (
        "@test",
        ".alice@test",
        "alice.@test",
        "a..b@test",
        "alice@-bad.test",
        "alice@bad-.test",
        "alice@a..test",
        "alice@тест",
        f"alice@{'a' * 64}",
    )

    with smtplib.SMTP(HOST, PORT, timeout=10) as client:
        client.starttls(context=tls_context)
        expect_reply(client.ehlo(), 250, "EHLO after STARTTLS")
        client.login("alice", os.environ["LITTERKEEP_ALICE_PASSWORD"])
        for mailbox in invalid_mailboxes:
            expect_reply(
                send_path_command(client, "MAIL FROM:", mailbox),
                501,
                f"invalid MAIL mailbox {mailbox!r}",
            )
        expect_reply(send_path_command(client, "MAIL FROM:", "alice@test"), 250, "valid MAIL mailbox")
        for mailbox in invalid_mailboxes:
            expect_reply(
                send_path_command(client, "RCPT TO:", mailbox),
                501,
                f"invalid RCPT mailbox {mailbox!r}",
            )
        expect_reply(send_path_command(client, "RCPT TO:", "bob@test"), 250, "valid RCPT mailbox")
        expect_reply(client.rset(), 250, "RSET after grammar checks")

    print("PASS smtp-mailbox-syntax: malformed dot-atoms/domains rejected in MAIL/RCPT")


if __name__ == "__main__":
    main()
