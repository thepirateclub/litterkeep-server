"""Проверяет базовые SMTP-команды независимым TCP-клиентом."""
import argparse
import socket


PARSER = argparse.ArgumentParser()
PARSER.add_argument("host", nargs="?", default="127.0.0.1")
PARSER.add_argument("port", nargs="?", type=int, default=2525)
OPTIONS = PARSER.parse_args()


def command(client, request, expected):
    client.sendall(request)
    response = client.recv(4096)
    assert response.startswith(expected), response


with socket.create_connection((OPTIONS.host, OPTIONS.port), timeout=10) as smtp:
    assert smtp.recv(4096).startswith(b"220")
    command(smtp, b"HELO sender.test\r\n", b"250 ")
    command(smtp, b"NOOP\r\n", b"250")
    command(smtp, b"MAIL FROM:<>\r\n", b"250")
    command(smtp, b"RSET\r\n", b"250")
    command(smtp, b"QUIT\r\n", b"221")

print("PASS smtp-basic-commands")
