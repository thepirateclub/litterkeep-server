"""Проверяет точную RFC 5321 форму начала MAIL и RCPT path."""

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


def exchange(client, request, expected_code):
    client.sendall(request)
    response = read_reply(client)
    if not response.startswith(expected_code):
        raise AssertionError(f"{request!r}: expected {expected_code!r}, got {response!r}")


with socket.create_connection((OPTIONS.host, OPTIONS.port), timeout=10) as smtp:
    if not read_reply(smtp).startswith(b"220"):
        raise AssertionError("server greeting was not 220")
    exchange(smtp, b"EHLO parser.test\r\n", b"250")
    for invalid in (
        b"MAIL FROM: <alice@test>\r\n",
        b"MAIL FROM:\t<alice@test>\r\n",
        b"RCPT TO: <bob@test>\r\n",
        b"RCPT TO:\t<bob@test>\r\n",
    ):
        exchange(smtp, invalid, b"501")
    exchange(smtp, b"MAIL FROM:<alice@test>\r\n", b"250")
    exchange(smtp, b"RCPT TO:<bob@test>\r\n", b"250")
    exchange(smtp, b"RSET\r\n", b"250")
    exchange(smtp, b"QUIT\r\n", b"221")

print("PASS smtp-path-colon-whitespace: invalid WSP rejected; valid path session recovers")
