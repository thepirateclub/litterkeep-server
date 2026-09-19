"""Независимая TCP-проверка, что IMAP чтение не доступно до LOGIN."""
import socket
import sys


HOST = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 2143
REQUEST = b"a1 SELECT INBOX\r\n"
REJECTION = b"a1 NO authenticate first\r\n"

with socket.create_connection((HOST, PORT), timeout=10) as client:
    greeting = client.recv(4096)
    assert greeting.startswith(b"* OK"), greeting
    client.sendall(REQUEST)
    response = client.recv(4096)

assert REJECTION in response, response
print("PASS IMAP SELECT requires LOGIN")
