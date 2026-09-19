"""Независимая TCP-проверка, что IMAP чтение не доступно до LOGIN."""
import argparse
import socket


PARSER = argparse.ArgumentParser()
PARSER.add_argument("host", nargs="?", default="127.0.0.1")
PARSER.add_argument("port", nargs="?", type=int, default=2143)
OPTIONS = PARSER.parse_args()
REQUEST = b"a1 SELECT INBOX\r\n"
REJECTION = b"a1 NO authenticate first\r\n"

with socket.create_connection((OPTIONS.host, OPTIONS.port), timeout=10) as client:
    greeting = client.recv(4096)
    assert greeting.startswith(b"* OK"), greeting
    client.sendall(REQUEST)
    response = client.recv(4096)

assert REJECTION in response, response
print("PASS IMAP SELECT requires LOGIN")
