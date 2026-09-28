"""Проверяет отказ от неподдерживаемого RFC 3461 параметра и recovery SMTP."""

import argparse
import socket


ARGUMENTS = argparse.ArgumentParser()
ARGUMENTS.add_argument("host", nargs="?", default="127.0.0.1")
ARGUMENTS.add_argument("port", nargs="?", type=int, default=2525)
OPTIONS = ARGUMENTS.parse_args()


def read_reply(client):
    """Читает один SMTP reply и возвращает его без интерпретации клиентским SDK."""
    response = client.recv(4096)
    if not response:
        raise AssertionError("SMTP peer closed before replying")
    return response


def exchange(client, request, expected):
    """Сверяет точный ожидаемый статус-код после отдельной команды."""
    client.sendall(request)
    response = read_reply(client)
    if not response.startswith(expected):
        raise AssertionError(f"{request!r}: expected {expected!r}, got {response!r}")
    return response


with socket.create_connection((OPTIONS.host, OPTIONS.port), timeout=10) as smtp:
    greeting = read_reply(smtp)
    if not greeting.startswith(b"220"):
        raise AssertionError(f"expected SMTP 220 greeting, got {greeting!r}")

    ehlo = exchange(smtp, b"EHLO dsn.test\r\n", b"250")
    if b"DSN" in ehlo.upper():
        raise AssertionError("server advertised the unsupported DSN extension")

    exchange(smtp, b"MAIL FROM:<alice@test>\r\n", b"250")
    refusal = exchange(
        smtp,
        b"RCPT TO:<bob@test> NOTIFY=SUCCESS\r\n",
        b"555 5.5.4",
    )
    if b"unsupported command parameter" not in refusal.lower():
        raise AssertionError(f"unexpected DSN refusal text: {refusal!r}")

    exchange(smtp, b"NOOP\r\n", b"250")
    exchange(smtp, b"RSET\r\n", b"250")
    exchange(smtp, b"QUIT\r\n", b"221")

print("PASS smtp-dsn-refusal: no DSN capability, exact 555, session recovery")
