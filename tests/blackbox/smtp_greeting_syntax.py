"""Проверяет EHLO/HELO syntax независимым raw-TCP SMTP-клиентом."""

import argparse
import socket


PARSER = argparse.ArgumentParser()
PARSER.add_argument("host", nargs="?", default="127.0.0.1")
PARSER.add_argument("port", nargs="?", type=int, default=2525)
OPTIONS = PARSER.parse_args()


def read_reply(client):
    response = client.recv(4096)
    if not response:
        raise AssertionError("SMTP peer closed before replying")
    return response


def exchange(client, request, expected):
    client.sendall(request)
    response = read_reply(client)
    if not response.startswith(expected):
        raise AssertionError(f"{request!r}: expected {expected!r}, got {response!r}")


with socket.create_connection((OPTIONS.host, OPTIONS.port), timeout=10) as smtp:
    if not read_reply(smtp).startswith(b"220"):
        raise AssertionError("server greeting was not 220")
    for invalid in (
        b"EHLO\r\n",
        b"HELO\r\n",
        b"EHLO sender.test relay.test\r\n",
        b"HELO sender.test relay.test\r\n",
        b"EHLO sender..test\r\n",
        b"HELO -sender.test\r\n",
        b"EHLO\tsender.test\r\n",
    ):
        exchange(smtp, invalid, b"501")
    exchange(smtp, b"EHLO sender.test\r\n", b"250")
    exchange(smtp, b"HELO sender.test\r\n", b"250")
    exchange(smtp, b"EHLO [IPv6:2001:db8::1]\r\n", b"250")
    exchange(smtp, b"QUIT\r\n", b"221")

print("PASS smtp-greeting-syntax: invalid greetings rejected; DNS and EHLO literal accepted")
