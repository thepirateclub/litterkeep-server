"""Проверяет SMTP mailbox grammar и сквозную локальную доставку."""

import imaplib
import os
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


def verify_mailbox_delivery():
    """Независимый IMAP-клиент проверяет оба доставленных сообщения Bob."""
    tls_context = ssl.create_default_context()
    tls_context.check_hostname = False
    tls_context.verify_mode = ssl.CERT_NONE
    client = imaplib.IMAP4("127.0.0.1", IMAP_PORT)
    client.starttls(ssl_context=tls_context)
    client.login("bob", os.environ["LITTERKEEP_BOB_PASSWORD"])
    status, _ = client.select("INBOX", readonly=True)
    if status != "OK":
        raise AssertionError(f"could not select Bob's INBOX: {status!r}")
    subjects = ("SMTP quoted local-part", "SMTP source-route ignored")
    for subject in subjects:
        status, response = client.search(None, "HEADER", "Subject", f'"{subject}"')
        message_ids = next(iter(response), b"").split()
        if status != "OK" or not message_ids:
            raise AssertionError(f"message {subject!r} was not stored in Bob's INBOX")
    client.logout()


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
        expect_reply(
            client.mail('@source.test,@relay.test:"alice"@litterkeep.test'),
            250,
            "source-routed MAIL sender",
        )
        expect_reply(
            client.rcpt('@source.test:"bob"@litterkeep.test'),
            250,
            "source-routed RCPT recipient",
        )
        expect_reply(
            client.data("Subject: SMTP source-route ignored\r\n\r\ncommitted\r\n"),
            250,
            "source-routed durable delivery",
        )
    verify_mailbox_delivery()

    print(
        "PASS smtp-mailbox-syntax: malformed paths rejected; quoted addresses "
        "and ignored source routes delivered"
    )


if __name__ == "__main__":
    main()
