"""Проверяет, что закрытая без терминатора SMTP DATA не публикуется."""
import imaplib
import socket
import sys


HOST = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1"
SMTP_PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 2525
IMAP_PORT = int(sys.argv[3]) if len(sys.argv) > 3 else 2143


def command(client, data, expected):
    client.sendall(data)
    response = client.recv(4096)
    assert response.startswith(expected), response


with socket.create_connection((HOST, SMTP_PORT), timeout=10) as smtp:
    banner = smtp.recv(4096)
    assert banner.startswith(b"220"), banner
    command(smtp, b"EHLO sender.test\r\n", b"250")
    command(smtp, b"MAIL FROM:<sender@test>\r\n", b"250")
    command(smtp, b"RCPT TO:<bob@test>\r\n", b"250")
    command(smtp, b"DATA\r\n", b"354")
    smtp.sendall(b"Subject: abandoned\r\n\r\nthis must not persist")

with imaplib.IMAP4(HOST, IMAP_PORT, timeout=10) as imap:
    status, _ = imap.login("bob", "test-password")
    assert status == "OK", status
    status, count = imap.select("INBOX", readonly=True)
    assert status == "OK", (status, count)
    assert int(next(iter(count))) == 0, count

print("PASS smtp-abandoned-data")
