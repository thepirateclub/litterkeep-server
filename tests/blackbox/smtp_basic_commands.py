"""Проверяет базовые SMTP-команды независимым TCP-клиентом."""
import socket
import sys


HOST = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 2525


def command(client, request, expected):
    client.sendall(request)
    response = client.recv(4096)
    assert response.startswith(expected), response


with socket.create_connection((HOST, PORT), timeout=10) as smtp:
    assert smtp.recv(4096).startswith(b"220")
    command(smtp, b"HELO sender.test\r\n", b"250 ")
    command(smtp, b"NOOP\r\n", b"250")
    command(smtp, b"MAIL FROM:<>\r\n", b"250")
    command(smtp, b"RSET\r\n", b"250")
    command(smtp, b"QUIT\r\n", b"221")

print("PASS smtp-basic-commands")
