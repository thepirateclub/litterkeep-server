"""Проверяет SMTP mailbox grammar и доставку quoted local-part."""

import os
import imaplib
import smtplib
import ssl


HOST = os.environ.get("LITTERKEEP_SMTP_HOST", "127.0.0.1")
PORT = int(os.environ.get("LITTERKEEP_SMTP_PORT", "2525"))
IMAP_PORT = int(os.environ.get("LITTERKEEP_IMAP_PORT", "1143"))


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


def verify_quoted_recipient_delivery():
    """Независимый IMAP-клиент подтверждает доставку в mailbox Bob."""
    tls_context = ssl.create_default_context()
    tls_context.check_hostname = False
    tls_context.verify_mode = ssl.CERT_NONE
    client = imaplib.IMAP4("127.0.0.1", IMAP_PORT)
    client.starttls(ssl_context=tls_context)
    client.login("bob", os.environ["LITTERKEEP_BOB_PASSWORD"])
    status, _ = client.select("INBOX", readonly=True)
    if status != "OK":
        raise AssertionError(f"could not select Bob's INBOX: {status!r}")
    status, response = client.search(None, "HEADER", "Subject", '"SMTP quoted local-part"')
    message_ids = next(iter(response), b"").split()
    client.logout()
    if status != "OK" or not message_ids:
        raise AssertionError("quoted RCPT message was not stored in Bob's INBOX")


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
        expect_reply(client.mail('"alice"@litterkeep.test'), 250, "quoted MAIL sender")
        expect_reply(client.rcpt('"bob"@litterkeep.test'), 250, "quoted RCPT recipient")
        expect_reply(
            client.data("Subject: SMTP quoted local-part\r\n\r\ncommitted\r\n"),
            250,
            "quoted mailbox durable delivery",
        )
    verify_quoted_recipient_delivery()

    print("PASS smtp-mailbox-syntax: malformed addresses rejected; quoted sender/recipient delivered")


if __name__ == "__main__":
    main()
