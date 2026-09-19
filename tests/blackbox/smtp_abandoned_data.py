"""Проверяет, что закрытая без терминатора SMTP DATA не публикуется."""
import argparse
import imaplib
import socket


PARSER = argparse.ArgumentParser()
PARSER.add_argument("host", nargs="?", default="127.0.0.1")
PARSER.add_argument("smtp_port", nargs="?", type=int, default=2525)
PARSER.add_argument("imap_port", nargs="?", type=int, default=2143)
OPTIONS = PARSER.parse_args()


def command(client, data, expected):
    client.sendall(data)
    response = client.recv(4096)
    assert response.startswith(expected), response


with socket.create_connection((OPTIONS.host, OPTIONS.smtp_port), timeout=10) as smtp:
    banner = smtp.recv(4096)
    assert banner.startswith(b"220"), banner
    command(smtp, b"EHLO sender.test\r\n", b"250")
    command(smtp, b"MAIL FROM:<sender@test>\r\n", b"250")
    command(smtp, b"RCPT TO:<bob@test>\r\n", b"250")
    command(smtp, b"DATA\r\n", b"354")
    smtp.sendall(b"Subject: abandoned\r\n\r\nthis must not persist")

with imaplib.IMAP4(OPTIONS.host, OPTIONS.imap_port, timeout=10) as imap:
    status, _ = imap.login("bob", "test-password")
    assert status == "OK", status
    status, count = imap.select("INBOX", readonly=True)
    assert status == "OK", (status, count)
    assert int(next(iter(count))) == 0, count

print("PASS smtp-abandoned-data")
